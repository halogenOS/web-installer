// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::future::Future;
use std::rc::Rc;

use byte_io::{ByteSource, SourceError};
use release::{PartitionName, Release, Step};
use sparse_image::SparseImageError;

use crate::checked_image::CheckedImage;
use crate::flash_plan::{flash_target, install_steps};
use crate::protocol::{Command, download_limit};
use crate::variable::YesNo;
use crate::{CheckedPhone, Fastboot, FastbootError, Link, Variable};

const NO_SNAPSHOT_UPDATE: &str = "none";

pub trait InstallHost {
    type Link: Link;

    fn image(&self, partition: &PartitionName) -> Result<Rc<dyn ByteSource>, SourceError>;

    fn super_empty(&self) -> Result<Rc<dyn ByteSource>, SourceError>;

    fn reconnect(
        &mut self,
    ) -> impl Future<Output = Result<Self::Link, <Self::Link as Link>::Error>>;

    fn report(&mut self, progress: InstallProgress<'_>);
}

#[derive(Clone, Copy, Debug)]
pub enum InstallProgress<'a> {
    Step {
        index: usize,
        count: usize,
        step: &'a Step,
    },
    Sent {
        sent: u64,
        total: u64,
    },
}

pub async fn install<H: InstallHost>(
    release: &Release,
    phone: &CheckedPhone,
    mut fastboot: Fastboot<H::Link>,
    host: &mut H,
) -> Result<(), FastbootError<<H::Link as Link>::Error>> {
    let steps = install_steps(release);
    let images = read_flashed_images(release, phone, &steps, host)?;
    for (index, &step) in steps.iter().enumerate() {
        host.report(InstallProgress::Step {
            index,
            count: steps.len(),
            step,
        });
        match step {
            Step::Flash { partition, logical } => {
                let target = flash_target(release, partition, &phone.slot);
                let image = images
                    .get(partition)
                    .expect("install reads every flashed image before the first step");
                let max_download_size = fastboot.require_size(&Variable::MaxDownloadSize).await?;
                let downloads = image.plan(download_limit(max_download_size)).ok_or(
                    FastbootError::CannotSend {
                        len: image.len(),
                        max_download: max_download_size,
                    },
                )?;
                if *logical {
                    fastboot
                        .run(Command::ResizeLogicalPartition {
                            partition: &target,
                            size: image.partition_len(),
                        })
                        .await?;
                }
                let total = downloads.sent_len();
                let mut sent = 0;
                let mut on_sent = |bytes| {
                    sent += bytes;
                    host.report(InstallProgress::Sent { sent, total });
                };
                fastboot.flash(&target, &downloads, &mut on_sent).await?;
            }
            Step::RebootFastboot => {
                fastboot.run(Command::RebootFastboot).await?;
                let link = host.reconnect().await.map_err(FastbootError::Link)?;
                fastboot = fastboot.reconnected(link);
                let is_userspace = fastboot.getvar(&Variable::IsUserspace).await?;
                if is_userspace.parse() != Ok(YesNo::Yes) {
                    return Err(FastbootError::NotFastbootd { is_userspace });
                }
            }
            Step::UpdateSuper { super_device, .. } => {
                if let Ok(status) = fastboot
                    .read_variable(&Variable::SnapshotUpdateStatus)
                    .await?
                    && status != NO_SNAPSHOT_UPDATE
                {
                    fastboot.run(Command::CancelSnapshotUpdate).await?;
                }
                let layout = host.super_empty().map_err(FastbootError::Read)?;
                fastboot.download(&*layout, &mut |_layout_bytes| {}).await?;
                fastboot
                    .run(Command::UpdateSuper(super_device.as_str()))
                    .await?;
            }
            Step::Erase { .. } => {}
        }
    }
    fastboot.run(Command::Reboot).await
}

/// Reads every image that the install flashes before the first step writes to the phone, so
/// that a broken image or one larger than its partition stops the install with the phone as it
/// was.
fn read_flashed_images<H: InstallHost>(
    release: &Release,
    phone: &CheckedPhone,
    steps: &[&Step],
    host: &H,
) -> Result<BTreeMap<PartitionName, CheckedImage>, FastbootError<<H::Link as Link>::Error>> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Flash { partition, .. } => Some(partition),
            Step::RebootFastboot | Step::UpdateSuper { .. } | Step::Erase { .. } => None,
        })
        .map(|partition| {
            let image = host.image(partition).map_err(FastbootError::Read)?;
            let image = CheckedImage::read(image).map_err(|error| match error {
                SparseImageError::ReadStart(source) => FastbootError::Read(source),
                source => FastbootError::SparseImage {
                    partition: partition.clone(),
                    source,
                },
            })?;
            let target = flash_target(release, partition, &phone.slot);
            if let Some(unfit) = phone.partition_sizes.unfit(&target, image.partition_len()) {
                return Err(FastbootError::PartitionUnfit(unfit));
            }
            Ok((partition.clone(), image))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::convert::Infallible;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use byte_io::{MemorySource, to_len};
    use sparse_image::{BLOCK_LEN, min_split_len};

    use super::*;
    use crate::UnfitPartition;
    use crate::phone_check::PartitionSizes;
    use crate::test_images::{blocks_len, sparse_image};

    const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[derive(Debug, Eq, PartialEq)]
    enum Sent {
        Command(String),
        BodyPiece(usize),
    }

    struct FakePhone {
        max_download_size: u64,
        sent: Rc<RefCell<Vec<Sent>>>,
        body_left: u64,
        replies: VecDeque<String>,
    }

    impl Link for FakePhone {
        type Error = Infallible;

        // A whole number of packets of 512 bytes, the bulk packet size of USB 2.0 high speed, and
        // short enough that a body of a few blocks goes out in several pieces.
        const SEND_LIMIT: usize = 4096;

        async fn send(&mut self, bytes: &[u8]) -> Result<(), Infallible> {
            if self.body_left > 0 {
                self.body_left -= to_len(bytes.len());
                self.sent.borrow_mut().push(Sent::BodyPiece(bytes.len()));
                if self.body_left == 0 {
                    self.replies.push_back("OKAY".to_owned());
                }
                return Ok(());
            }
            let command = String::from_utf8(bytes.to_vec()).unwrap();
            let reply = if command == "getvar:max-download-size" {
                format!("OKAY{:#x}", self.max_download_size)
            } else if let Some(len) = command.strip_prefix("download:") {
                self.body_left = u64::from_str_radix(len, 16).unwrap();
                format!("DATA{len}")
            } else {
                "OKAY".to_owned()
            };
            self.replies.push_back(reply);
            self.sent.borrow_mut().push(Sent::Command(command));
            Ok(())
        }

        async fn receive(&mut self, _limit: u32) -> Result<Vec<u8>, Infallible> {
            Ok(self
                .replies
                .pop_front()
                .expect("the installer reads a reply only after a command or a body")
                .into_bytes())
        }
    }

    struct StoredImages {
        boot: Vec<u8>,
        system: Vec<u8>,
    }

    impl InstallHost for StoredImages {
        type Link = FakePhone;

        fn image(&self, partition: &PartitionName) -> Result<Rc<dyn ByteSource>, SourceError> {
            let bytes = match partition.as_str() {
                "boot" => &self.boot,
                "system" => &self.system,
                other => panic!("the release flashes no image of {other}"),
            };
            Ok(Rc::new(MemorySource::new(bytes.clone())))
        }

        fn super_empty(&self) -> Result<Rc<dyn ByteSource>, SourceError> {
            panic!("the release has no update-super step")
        }

        async fn reconnect(&mut self) -> Result<FakePhone, Infallible> {
            panic!("the release has no reboot-fastboot step")
        }

        fn report(&mut self, _progress: InstallProgress<'_>) {}
    }

    /// A release that flashes the physical partition boot, then the logical partition system.
    fn release() -> Release {
        Release::from_json(&format!(
            r#"{{
                "format": 1,
                "device": "spacewar",
                "build": "build",
                "requirements": [{{"variable": "product", "values": ["Spacewar"]}}],
                "sources": [],
                "images": {{
                    "boot": {{"kind": "shipped", "sha256": "{DIGEST}", "size": 1}},
                    "system": {{"kind": "shipped", "sha256": "{DIGEST}", "size": 1}}
                }},
                "install": [
                    {{"step": "flash", "partition": "boot", "logical": false}},
                    {{"step": "flash", "partition": "system", "logical": true}}
                ],
                "package": {{
                    "facts": {{
                        "build": {{
                            "device": "Spacewar",
                            "fingerprint": "fingerprint",
                            "incremental": "1",
                            "sdk_level": 37,
                            "security_patch_level": "2026-11-01",
                            "timestamp": 1
                        }},
                        "super_partition": {{
                            "groups": [{{"name": "main", "size": 1000000, "partitions": ["system"]}}],
                            "snapshots": false
                        }},
                        "ota_certificate": "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n"
                    }},
                    "partitions": {{"boot": [], "system": []}},
                    "payload_crc32": 0,
                    "signatures": {{"metadata": "AA==", "payload": "AA==", "whole_file": "AA=="}},
                    "sha256": "{DIGEST}",
                    "size": 1
                }}
            }}"#
        ))
        .unwrap()
    }

    struct Run {
        result: Result<(), FastbootError<Infallible>>,
        sent: Vec<Sent>,
    }

    fn run_install(images: StoredImages, max_download_size: u64, boot_size: u64) -> Run {
        let sent = Rc::new(RefCell::new(Vec::new()));
        let phone = FakePhone {
            max_download_size,
            sent: Rc::clone(&sent),
            body_left: 0,
            replies: VecDeque::new(),
        };
        let checked = CheckedPhone {
            slot: "a".to_owned(),
            partition_sizes: PartitionSizes(BTreeMap::from([("boot_a".to_owned(), boot_size)])),
        };
        let mut host = images;
        let release = release();
        let future = pin!(install(
            &release,
            &checked,
            Fastboot::new(phone, Box::new(|_message| {})),
            &mut host
        ));
        let Poll::Ready(result) = future.poll(&mut Context::from_waker(Waker::noop())) else {
            panic!("the fake phone replies at once");
        };
        Run {
            result,
            sent: sent.take(),
        }
    }

    fn commands(sent: &[Sent]) -> Vec<&str> {
        sent.iter()
            .filter_map(|sent| match sent {
                Sent::Command(command) => Some(command.as_str()),
                Sent::BodyPiece(_) => None,
            })
            .collect()
    }

    fn images(system: Vec<u8>) -> StoredImages {
        StoredImages {
            boot: vec![1; 100],
            system,
        }
    }

    #[test]
    fn a_logical_partition_is_resized_to_the_blocks_that_its_sparse_image_describes() {
        let run = run_install(images(sparse_image(3, 1)), 0x1000_0000, 100);
        run.result.unwrap();
        let resize = format!("resize-logical-partition:system_a:{}", blocks_len(4));
        assert!(commands(&run.sent).contains(&resize.as_str()));
    }

    #[test]
    fn a_split_sparse_image_goes_out_in_downloads_of_whole_send_pieces() {
        let max_download_size = min_split_len(BLOCK_LEN);
        let run = run_install(images(sparse_image(3, 1)), max_download_size, 100);
        run.result.unwrap();
        let flashed = commands(&run.sent)
            .into_iter()
            .filter(|command| command.starts_with("flash:"))
            .count();
        assert_eq!(flashed, 1 + 3);
        for body in run
            .sent
            .split(|sent| matches!(sent, Sent::Command(_)))
            .filter(|body| !body.is_empty())
        {
            let (last, before) = body.split_last().unwrap();
            assert!(before.iter().all(|piece| *piece == Sent::BodyPiece(4096)));
            assert!(matches!(last, Sent::BodyPiece(len) if *len <= 4096));
        }
    }

    #[test]
    fn a_broken_sparse_image_stops_the_install_before_the_first_command() {
        let mut system = sparse_image(1, 1);
        system[4] = 9;
        let run = run_install(images(system), 0x1000_0000, 100);
        assert!(matches!(
            run.result,
            Err(FastbootError::SparseImage {
                partition,
                source: SparseImageError::MajorVersion(9),
            }) if partition.as_str() == "system"
        ));
        assert_eq!(run.sent, []);
    }

    #[test]
    fn a_sparse_image_that_describes_more_than_its_partition_stops_before_the_first_command() {
        let images = StoredImages {
            boot: sparse_image(1, 2),
            system: sparse_image(1, 1),
        };
        let run = run_install(images, 0x1000_0000, blocks_len(2));
        assert!(matches!(
            run.result,
            Err(FastbootError::PartitionUnfit(UnfitPartition {
                partition,
                image_size,
                partition_size,
            })) if partition == "boot_a" && image_size == blocks_len(3) && partition_size == blocks_len(2)
        ));
        assert_eq!(run.sent, []);
    }

    #[test]
    fn a_download_too_short_for_one_block_stops_before_the_resize() {
        let run = run_install(
            images(sparse_image(3, 1)),
            min_split_len(BLOCK_LEN) - 1,
            100,
        );
        assert!(matches!(run.result, Err(FastbootError::CannotSend { .. })));
        assert!(
            !commands(&run.sent)
                .iter()
                .any(|command| command.starts_with("resize-logical-partition:"))
        );
    }
}
