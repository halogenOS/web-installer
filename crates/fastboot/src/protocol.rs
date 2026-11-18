// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use byte_io::ByteSource;

use crate::sparse::{Segment, downloads};
use crate::{FastbootError, Link};

const REPLY_LEN: u32 = 256;
const REPLY_CODE_LEN: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command<'a> {
    GetVar(&'a str),
    Download(u32),
    Flash(&'a str),
    Reboot,
    RebootFastboot,
    ResizeLogicalPartition { partition: &'a str, size: u64 },
    UpdateSuper(&'a str),
    CancelSnapshotUpdate,
}

impl Command<'_> {
    fn text(self) -> String {
        match self {
            Self::GetVar(name) => format!("getvar:{name}"),
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

    pub async fn getvar(&mut self, name: &str) -> Result<String, FastbootError<L::Error>> {
        self.okay(Command::GetVar(name)).await
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
        image: &dyn ByteSource,
        max_download: u64,
        on_sent: &mut dyn FnMut(u64),
    ) -> Result<(), FastbootError<L::Error>> {
        let len = image.len();
        let pieces = downloads(len, max_download.min(u64::from(u32::MAX)))
            .ok_or(FastbootError::CannotSend { len, max_download })?;
        for piece in pieces {
            self.download(&piece, image, on_sent).await?;
            self.run(Command::Flash(partition)).await?;
        }
        Ok(())
    }

    pub(crate) async fn download_whole(
        &mut self,
        image: &dyn ByteSource,
        on_sent: &mut dyn FnMut(u64),
    ) -> Result<(), FastbootError<L::Error>> {
        let segment = Segment::Image {
            start: 0,
            len: image.len(),
        };
        self.download(&[segment], image, on_sent).await
    }

    async fn download(
        &mut self,
        segments: &[Segment],
        image: &dyn ByteSource,
        on_sent: &mut dyn FnMut(u64),
    ) -> Result<(), FastbootError<L::Error>> {
        let total: u64 = segments
            .iter()
            .map(|segment| match segment {
                Segment::Bytes(bytes) => bytes.len() as u64,
                Segment::Image { len, .. } => *len,
            })
            .sum();
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
        for segment in segments {
            match segment {
                Segment::Bytes(bytes) => {
                    self.link.send(bytes).await.map_err(FastbootError::Link)?;
                }
                Segment::Image { start, len } => {
                    let mut chunk = vec![0; usize::try_from(*len).expect("a chunk fits in memory")];
                    image
                        .read_at(*start, &mut chunk)
                        .map_err(FastbootError::Read)?;
                    self.link.send(&chunk).await.map_err(FastbootError::Link)?;
                    on_sent(*len);
                }
            }
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
