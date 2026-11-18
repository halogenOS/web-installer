// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use byte_io::{ByteSource, clamp_len, to_len};

use crate::checked_image::ImageDownloads;
use crate::{FastbootError, Link, Variable};

const REPLY_LEN: u32 = 256;
const REPLY_CODE_LEN: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command<'a> {
    GetVar(&'a Variable),
    Download(u32),
    Flash(&'a str),
    Reboot,
    RebootFastboot,
    ResizeLogicalPartition { partition: &'a str, size: u64 },
    UpdateSuper(&'a str),
    CancelSnapshotUpdate,
}

impl Command<'_> {
    pub(crate) fn text(self) -> String {
        match self {
            Self::GetVar(variable) => format!("getvar:{variable}"),
            Self::Download(len) => format!("download:{len:08x}"),
            Self::Flash(partition) => format!("flash:{partition}"),
            Self::Reboot => "reboot".to_owned(),
            Self::RebootFastboot => "reboot-fastboot".to_owned(),
            Self::ResizeLogicalPartition { partition, size } => {
                format!("resize-logical-partition:{partition}:{size}")
            }
            Self::UpdateSuper(partition) => format!("update-super:{partition}"),
            Self::CancelSnapshotUpdate => "snapshot-update:cancel".to_owned(),
        }
    }
}

enum Completion {
    Okay(String),
    Data(u32),
}

impl Completion {
    fn reply(&self) -> String {
        match self {
            Self::Okay(text) => format!("OKAY{text}"),
            Self::Data(len) => format!("DATA{len:08x}"),
        }
    }
}

pub struct Fastboot<L> {
    link: L,
    on_message: Box<dyn FnMut(&str)>,
}

impl<L: Link> Fastboot<L> {
    pub fn new(link: L, on_message: Box<dyn FnMut(&str)>) -> Self {
        Self { link, on_message }
    }

    pub(crate) async fn getvar(
        &mut self,
        variable: &Variable,
    ) -> Result<String, FastbootError<L::Error>> {
        self.okay(Command::GetVar(variable)).await
    }

    pub(crate) async fn run(
        &mut self,
        command: Command<'_>,
    ) -> Result<(), FastbootError<L::Error>> {
        let _text_after_okay = self.okay(command).await?;
        Ok(())
    }

    pub(crate) async fn flash(
        &mut self,
        partition: &str,
        downloads: &ImageDownloads,
        on_sent: &mut dyn FnMut(u64),
    ) -> Result<(), FastbootError<L::Error>> {
        for body in downloads.bodies() {
            self.download(&**body, on_sent).await?;
            self.run(Command::Flash(partition)).await?;
        }
        Ok(())
    }

    pub(crate) async fn download(
        &mut self,
        body: &dyn ByteSource,
        on_sent: &mut dyn FnMut(u64),
    ) -> Result<(), FastbootError<L::Error>> {
        let total = body.len();
        let len = u32::try_from(total).map_err(|_| FastbootError::CannotSend {
            len: total,
            max_download: u64::from(u32::MAX),
        })?;
        let command = Command::Download(len);
        match self.complete(command).await? {
            Completion::Data(announced) if announced == len => {}
            other => {
                return Err(FastbootError::UnexpectedReply {
                    command: command.text(),
                    reply: other.reply(),
                });
            }
        }
        let mut buffer = vec![0; clamp_len(total, L::SEND_LIMIT)];
        for offset in (0..total).step_by(L::SEND_LIMIT) {
            let piece = &mut buffer[..clamp_len(total - offset, L::SEND_LIMIT)];
            body.read_at(offset, piece).map_err(FastbootError::Read)?;
            self.link.send(piece).await.map_err(FastbootError::Link)?;
            on_sent(to_len(piece.len()));
        }
        match self.reply(command).await? {
            Completion::Okay(_) => Ok(()),
            other @ Completion::Data(_) => Err(FastbootError::UnexpectedReply {
                command: command.text(),
                reply: other.reply(),
            }),
        }
    }

    async fn okay(&mut self, command: Command<'_>) -> Result<String, FastbootError<L::Error>> {
        match self.complete(command).await? {
            Completion::Okay(text) => Ok(text),
            other @ Completion::Data(_) => Err(FastbootError::UnexpectedReply {
                command: command.text(),
                reply: other.reply(),
            }),
        }
    }

    async fn complete(
        &mut self,
        command: Command<'_>,
    ) -> Result<Completion, FastbootError<L::Error>> {
        self.link
            .send(command.text().as_bytes())
            .await
            .map_err(FastbootError::Link)?;
        self.reply(command).await
    }

    async fn reply(&mut self, command: Command<'_>) -> Result<Completion, FastbootError<L::Error>> {
        loop {
            let reply = self
                .link
                .receive(REPLY_LEN)
                .await
                .map_err(FastbootError::Link)?;
            let text = String::from_utf8_lossy(reply.get(REPLY_CODE_LEN..).unwrap_or_default())
                .trim_end_matches('\0')
                .to_owned();
            let unexpected = || FastbootError::UnexpectedReply {
                command: command.text(),
                reply: String::from_utf8_lossy(&reply).into_owned(),
            };
            return match reply.get(..REPLY_CODE_LEN) {
                Some(b"OKAY") => Ok(Completion::Okay(text)),
                Some(b"INFO" | b"TEXT") => {
                    (self.on_message)(&text);
                    continue;
                }
                Some(b"FAIL") => Err(FastbootError::Failed {
                    command: command.text(),
                    message: text,
                }),
                Some(b"DATA") => u32::from_str_radix(&text, 16)
                    .map(Completion::Data)
                    .map_err(|_| unexpected()),
                _ => Err(unexpected()),
            };
        }
    }

    #[must_use]
    pub fn reconnected(self, link: L) -> Self {
        Self {
            link,
            on_message: self.on_message,
        }
    }
}

pub(crate) fn download_limit(max_download_size: u64) -> u32 {
    u32::try_from(max_download_size).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::convert::Infallible;
    use std::pin::pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    use super::*;

    struct ReplyingPhone {
        packets: VecDeque<Vec<u8>>,
    }

    impl Link for ReplyingPhone {
        type Error = Infallible;

        // The phone takes commands only, and a fastboot command is at most FB_COMMAND_SZ bytes
        // (system/core/fastboot/constants.h of android-17.0.0_r1, checked 2026-11-18).
        const SEND_LIMIT: usize = 4096;

        async fn send(&mut self, _bytes: &[u8]) -> Result<(), Infallible> {
            Ok(())
        }

        async fn receive(&mut self, _limit: u32) -> Result<Vec<u8>, Infallible> {
            Ok(self
                .packets
                .pop_front()
                .expect("the reply ends before the phone runs out of packets"))
        }
    }

    struct Reply {
        completion: Result<Completion, FastbootError<Infallible>>,
        messages: Vec<String>,
    }

    fn read_reply(packets: &[&[u8]]) -> Reply {
        let messages = Rc::new(RefCell::new(Vec::new()));
        let shown = Rc::clone(&messages);
        let phone = ReplyingPhone {
            packets: packets.iter().map(|packet| packet.to_vec()).collect(),
        };
        let mut fastboot = Fastboot::new(
            phone,
            Box::new(move |message| shown.borrow_mut().push(message.to_owned())),
        );
        let future = pin!(fastboot.reply(Command::Reboot));
        let Poll::Ready(completion) = future.poll(&mut Context::from_waker(Waker::noop())) else {
            panic!("the phone returns every packet at once");
        };
        Reply {
            completion,
            messages: messages.take(),
        }
    }

    fn is_unexpected_reply(
        completion: &Result<Completion, FastbootError<Infallible>>,
        packet: &str,
    ) -> bool {
        matches!(
            completion,
            Err(FastbootError::UnexpectedReply { command, reply }) if command == "reboot" && reply == packet
        )
    }

    #[test]
    fn okay_completes_with_the_text_after_its_code_without_trailing_nul_bytes() {
        let reply = read_reply(&[b"OKAY0x1000\0\0"]);
        assert!(matches!(reply.completion, Ok(Completion::Okay(text)) if text == "0x1000"));
    }

    #[test]
    fn okay_without_text_completes_with_empty_text() {
        let reply = read_reply(&[b"OKAY"]);
        assert!(matches!(reply.completion, Ok(Completion::Okay(text)) if text.is_empty()));
    }

    #[test]
    fn info_and_text_packets_go_to_the_messages_in_order_and_the_reply_goes_on() {
        let reply = read_reply(&[b"INFOerasing", b"TEXTwriting\0", b"OKAY"]);
        assert_eq!(reply.messages, ["erasing", "writing"]);
        assert!(matches!(reply.completion, Ok(Completion::Okay(_))));
    }

    #[test]
    fn fail_ends_the_reply_with_the_command_and_the_message_of_the_phone() {
        let reply = read_reply(&[b"INFOchecking", b"FAILpartition does not exist"]);
        assert_eq!(reply.messages, ["checking"]);
        assert!(matches!(
            reply.completion,
            Err(FastbootError::Failed { command, message })
                if command == "reboot" && message == "partition does not exist"
        ));
    }

    #[test]
    fn data_completes_with_the_length_read_as_hexadecimal() {
        let reply = read_reply(&[b"DATA0001f000"]);
        assert!(matches!(reply.completion, Ok(Completion::Data(0x1_f000))));
    }

    #[test]
    fn data_without_a_hexadecimal_length_is_an_unexpected_reply() {
        let reply = read_reply(&[b"DATAlength"]);
        assert!(is_unexpected_reply(&reply.completion, "DATAlength"));
    }

    #[test]
    fn data_with_a_length_beyond_32_bits_is_an_unexpected_reply() {
        let reply = read_reply(&[b"DATA100000000"]);
        assert!(is_unexpected_reply(&reply.completion, "DATA100000000"));
    }

    #[test]
    fn an_unknown_code_is_an_unexpected_reply_holding_the_whole_packet() {
        let reply = read_reply(&[b"BUSYwait"]);
        assert!(is_unexpected_reply(&reply.completion, "BUSYwait"));
    }

    #[test]
    fn a_code_in_lower_case_is_an_unexpected_reply() {
        let reply = read_reply(&[b"okay"]);
        assert!(is_unexpected_reply(&reply.completion, "okay"));
    }

    #[test]
    fn a_packet_shorter_than_a_code_is_an_unexpected_reply() {
        let reply = read_reply(&[b"OK"]);
        assert!(is_unexpected_reply(&reply.completion, "OK"));
    }

    #[test]
    fn an_empty_packet_is_an_unexpected_reply() {
        let reply = read_reply(&[b""]);
        assert!(is_unexpected_reply(&reply.completion, ""));
    }
}
