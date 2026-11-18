// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;

use release::{Release, RequiredVariable, Step};

use sparse_image::{BLOCK_LEN, min_split_len};

use crate::flash_plan::{flash_target, install_steps};
use crate::protocol::download_limit;
use crate::variable::YesNo;
use crate::{Fastboot, FastbootError, Link, Unreported, Variable};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PassedCheck {
    Model { product: String },
    Bootloader,
    Battery,
    Slot { slot: String },
    Partitions { count: usize },
    TransferSize { download_limit: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhoneCheckFailure {
    ProductUnreported(Unreported),
    OtherProduct {
        product: String,
    },
    RequirementsUnmet {
        product: String,
        unmet: Vec<UnmetRequirement>,
    },
    UnlockStateUnreported(Unreported),
    BootloaderLocked,
    BatteryUnreported(Unreported),
    BatteryLow,
    SlotUnreported(Unreported),
    PartitionSizeUnreported(Unreported),
    PartitionsUnfit {
        unfit: Vec<UnfitPartition>,
        count: usize,
    },
    TransferSizeUnreported(Unreported),
    TransferSizeTooSmall {
        download_limit: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnmetRequirement {
    pub variable: RequiredVariable,
    pub reported: Option<String>,
    pub values: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnfitPartition {
    pub partition: String,
    pub image_size: u64,
    pub partition_size: u64,
}

pub type PhoneCheck<'a> = Result<&'a PassedCheck, &'a PhoneCheckFailure>;

#[derive(Debug)]
pub struct CheckedPhone {
    pub(crate) slot: String,
    pub(crate) partition_sizes: PartitionSizes,
}

/// The sizes that the phone reported for the physical partitions that the install flashes, by
/// flash target.
#[derive(Debug, Default)]
pub(crate) struct PartitionSizes(pub(crate) BTreeMap<String, u64>);

impl PartitionSizes {
    /// The partition `target` with its size, when the phone reported one and an image that
    /// writes `image_size` bytes does not fit it.
    pub(crate) fn unfit(&self, target: &str, image_size: u64) -> Option<UnfitPartition> {
        let partition_size = *self.0.get(target)?;
        (image_size > partition_size).then(|| UnfitPartition {
            partition: target.to_owned(),
            image_size,
            partition_size,
        })
    }
}

enum Stop<E> {
    Phone(FastbootError<E>),
    Failed(PhoneCheckFailure),
}

impl<E> From<FastbootError<E>> for Stop<E> {
    fn from(error: FastbootError<E>) -> Self {
        Self::Phone(error)
    }
}

impl<E> From<PhoneCheckFailure> for Stop<E> {
    fn from(failure: PhoneCheckFailure) -> Self {
        Self::Failed(failure)
    }
}

pub async fn check_phone<L: Link>(
    fastboot: &mut Fastboot<L>,
    release: &Release,
    on_check: &mut dyn FnMut(PhoneCheck<'_>),
) -> Result<Result<CheckedPhone, PhoneCheckFailure>, FastbootError<L::Error>> {
    match run_checks(fastboot, release, on_check).await {
        Ok(phone) => Ok(Ok(phone)),
        Err(Stop::Failed(failure)) => Ok(Err(failure)),
        Err(Stop::Phone(error)) => Err(error),
    }
}

async fn run_checks<L: Link>(
    fastboot: &mut Fastboot<L>,
    release: &Release,
    on_check: &mut dyn FnMut(PhoneCheck<'_>),
) -> Result<CheckedPhone, Stop<L::Error>> {
    record(on_check, check_model(fastboot, release).await)?;
    record(on_check, check_bootloader(fastboot).await)?;
    record(on_check, check_battery(fastboot).await)?;
    let slot = read_slot(fastboot)
        .await
        .inspect_err(|stop| report_failure(on_check, stop))?;
    on_check(Ok(&PassedCheck::Slot { slot: slot.clone() }));
    let (partitions, partition_sizes) = check_partitions(fastboot, release, &slot)
        .await
        .inspect_err(|stop| report_failure(on_check, stop))?;
    on_check(Ok(&partitions));
    record(on_check, check_transfer_size(fastboot).await)?;
    Ok(CheckedPhone {
        slot,
        partition_sizes,
    })
}

fn record<E>(
    on_check: &mut dyn FnMut(PhoneCheck<'_>),
    check: Result<PassedCheck, Stop<E>>,
) -> Result<(), Stop<E>> {
    let passed = check.inspect_err(|stop| report_failure(on_check, stop))?;
    on_check(Ok(&passed));
    Ok(())
}

fn report_failure<E>(on_check: &mut dyn FnMut(PhoneCheck<'_>), stop: &Stop<E>) {
    if let Stop::Failed(failure) = stop {
        on_check(Err(failure));
    }
}

async fn check_model<L: Link>(
    fastboot: &mut Fastboot<L>,
    release: &Release,
) -> Result<PassedCheck, Stop<L::Error>> {
    let product = fastboot
        .read_variable(&Variable::Required(RequiredVariable::Product))
        .await?
        .map_err(PhoneCheckFailure::ProductUnreported)?;
    if !release.parts().accepts_product(&product) {
        return Err(PhoneCheckFailure::OtherProduct { product }.into());
    }
    let mut unmet = Vec::new();
    for requirement in release
        .parts()
        .requirements
        .iter()
        .filter(|requirement| requirement.variable != RequiredVariable::Product)
    {
        let reported = fastboot
            .read_variable(&Variable::Required(requirement.variable))
            .await?
            .ok();
        if !reported
            .as_ref()
            .is_some_and(|value| requirement.values.contains(value))
        {
            unmet.push(UnmetRequirement {
                variable: requirement.variable,
                reported,
                values: requirement.values.clone(),
            });
        }
    }
    if unmet.is_empty() {
        Ok(PassedCheck::Model { product })
    } else {
        Err(PhoneCheckFailure::RequirementsUnmet { product, unmet }.into())
    }
}

async fn check_bootloader<L: Link>(
    fastboot: &mut Fastboot<L>,
) -> Result<PassedCheck, Stop<L::Error>> {
    match fastboot
        .read_answer(&Variable::Unlocked)
        .await?
        .map_err(PhoneCheckFailure::UnlockStateUnreported)?
    {
        YesNo::Yes => Ok(PassedCheck::Bootloader),
        YesNo::No => Err(PhoneCheckFailure::BootloaderLocked.into()),
    }
}

async fn check_battery<L: Link>(fastboot: &mut Fastboot<L>) -> Result<PassedCheck, Stop<L::Error>> {
    match fastboot
        .read_answer(&Variable::BatterySocOk)
        .await?
        .map_err(PhoneCheckFailure::BatteryUnreported)?
    {
        YesNo::Yes => Ok(PassedCheck::Battery),
        YesNo::No => Err(PhoneCheckFailure::BatteryLow.into()),
    }
}

async fn read_slot<L: Link>(fastboot: &mut Fastboot<L>) -> Result<String, Stop<L::Error>> {
    let slot = fastboot
        .read_variable(&Variable::CurrentSlot)
        .await?
        .map_err(PhoneCheckFailure::SlotUnreported)?;
    Ok(slot.trim_start_matches('_').to_owned())
}

async fn check_partitions<L: Link>(
    fastboot: &mut Fastboot<L>,
    release: &Release,
    slot: &str,
) -> Result<(PassedCheck, PartitionSizes), Stop<L::Error>> {
    let steps = install_steps(release);
    let flashed: Vec<_> = release
        .parts()
        .images
        .iter()
        .filter(|(partition, _)| {
            steps.iter().any(|step| {
                matches!(step, Step::Flash { partition: flashed, logical: false } if flashed == *partition)
            })
        })
        .collect();
    let mut sizes = PartitionSizes::default();
    for (partition, _) in &flashed {
        let target = flash_target(release, partition, slot);
        let partition_size = fastboot
            .read_size(&Variable::PartitionSize(target.clone()))
            .await?
            .map_err(PhoneCheckFailure::PartitionSizeUnreported)?;
        let _earlier_size = sizes.0.insert(target, partition_size);
    }
    let unfit: Vec<UnfitPartition> = flashed
        .iter()
        .filter_map(|(partition, image)| {
            sizes.unfit(&flash_target(release, partition, slot), image.size())
        })
        .collect();
    let count = flashed.len();
    if unfit.is_empty() {
        Ok((PassedCheck::Partitions { count }, sizes))
    } else {
        Err(PhoneCheckFailure::PartitionsUnfit { unfit, count }.into())
    }
}

async fn check_transfer_size<L: Link>(
    fastboot: &mut Fastboot<L>,
) -> Result<PassedCheck, Stop<L::Error>> {
    let max_download_size = fastboot
        .read_size(&Variable::MaxDownloadSize)
        .await?
        .map_err(PhoneCheckFailure::TransferSizeUnreported)?;
    let download_limit = download_limit(max_download_size);
    if u64::from(download_limit) >= min_split_len(BLOCK_LEN) {
        Ok(PassedCheck::TransferSize { download_limit })
    } else {
        Err(PhoneCheckFailure::TransferSizeTooSmall { download_limit }.into())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::convert::Infallible;
    use std::pin::pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    use super::*;

    const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    struct FakePhone {
        replies: HashMap<String, String>,
        sent: Rc<RefCell<Vec<String>>>,
        reply: Vec<u8>,
    }

    impl Link for FakePhone {
        type Error = Infallible;

        // The fake phone takes commands only, and a fastboot command is at most FB_COMMAND_SZ bytes
        // (system/core/fastboot/constants.h of android-17.0.0_r1, checked 2026-11-18).
        const SEND_LIMIT: usize = 4096;

        async fn send(&mut self, bytes: &[u8]) -> Result<(), Infallible> {
            let command = String::from_utf8(bytes.to_vec()).unwrap();
            self.reply = self
                .replies
                .get(&command)
                .map_or("FAILunknown variable", String::as_str)
                .as_bytes()
                .to_vec();
            self.sent.borrow_mut().push(command);
            Ok(())
        }

        async fn receive(&mut self, _limit: u32) -> Result<Vec<u8>, Infallible> {
            Ok(std::mem::take(&mut self.reply))
        }
    }

    struct Run {
        result: Result<CheckedPhone, PhoneCheckFailure>,
        checks: Vec<Result<PassedCheck, PhoneCheckFailure>>,
        sent: Vec<String>,
    }

    fn release() -> Release {
        let images = format!(
            r#"{{"boot": {{"kind": "shipped", "sha256": "{DIGEST}", "size": 100}},
                "dtbo": {{"kind": "shipped", "sha256": "{DIGEST}", "size": 50}}}}"#
        );
        Release::from_json(&format!(
            r#"{{
                "format": 1,
                "device": "spacewar",
                "build": "build",
                "requirements": [
                    {{"variable": "product", "values": ["Spacewar"]}},
                    {{"variable": "version-baseband", "values": ["1.0"]}}
                ],
                "sources": [],
                "images": {images},
                "install": [
                    {{"step": "flash", "partition": "boot", "logical": false}},
                    {{"step": "flash", "partition": "dtbo", "logical": false}}
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
                        "ota_certificate": "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n"
                    }},
                    "partitions": {{"boot": []}},
                    "payload_crc32": 0,
                    "signatures": {{"metadata": "AA==", "payload": "AA==", "whole_file": "AA=="}},
                    "sha256": "{DIGEST}",
                    "size": 1
                }}
            }}"#
        ))
        .unwrap()
    }

    fn ready_phone() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("product", "OKAYSpacewar"),
            ("version-baseband", "OKAY1.0"),
            ("unlocked", "OKAYyes"),
            ("battery-soc-ok", "OKAYyes"),
            ("current-slot", "OKAY_a"),
            ("partition-size:boot_a", "OKAY0x100"),
            ("partition-size:dtbo", "OKAY50"),
            ("max-download-size", "OKAY0x10000000"),
        ])
    }

    fn check(replies: &HashMap<&str, &str>) -> Run {
        let sent = Rc::new(RefCell::new(Vec::new()));
        let phone = FakePhone {
            replies: replies
                .iter()
                .map(|(variable, reply)| (format!("getvar:{variable}"), (*reply).to_owned()))
                .collect(),
            sent: Rc::clone(&sent),
            reply: Vec::new(),
        };
        let mut fastboot = Fastboot::new(phone, Box::new(|_message| {}));
        let release = release();
        let mut checks = Vec::new();
        let mut record = |check: PhoneCheck<'_>| {
            checks.push(check.cloned().map_err(Clone::clone));
        };
        let result = {
            let future = pin!(check_phone(&mut fastboot, &release, &mut record));
            let Poll::Ready(Ok(result)) = future.poll(&mut Context::from_waker(Waker::noop()))
            else {
                panic!("the fake phone returns every reply at once");
            };
            result
        };
        Run {
            result,
            checks,
            sent: sent.take(),
        }
    }

    fn unreported(variable: Variable, refusal: Option<&str>) -> Unreported {
        Unreported {
            variable,
            refusal: refusal.map(str::to_owned),
        }
    }

    #[test]
    fn a_ready_phone_passes_every_check_and_receives_only_getvar() {
        let run = check(&ready_phone());
        assert_eq!(run.result.unwrap().slot, "a");
        assert_eq!(
            run.checks,
            [
                Ok(PassedCheck::Model {
                    product: "Spacewar".to_owned()
                }),
                Ok(PassedCheck::Bootloader),
                Ok(PassedCheck::Battery),
                Ok(PassedCheck::Slot {
                    slot: "a".to_owned()
                }),
                Ok(PassedCheck::Partitions { count: 2 }),
                Ok(PassedCheck::TransferSize {
                    download_limit: 0x1000_0000
                }),
            ]
        );
        assert!(
            run.sent
                .iter()
                .all(|command| command.starts_with("getvar:"))
        );
    }

    #[test]
    fn a_locked_bootloader_stops_before_the_next_variable_is_read() {
        let mut replies = ready_phone();
        let _yes = replies.insert("unlocked", "OKAYno");
        let run = check(&replies);
        assert_eq!(run.result.unwrap_err(), PhoneCheckFailure::BootloaderLocked);
        assert_eq!(run.sent.last().unwrap(), "getvar:unlocked");
        assert_eq!(
            run.checks.last(),
            Some(&Err(PhoneCheckFailure::BootloaderLocked))
        );
    }

    #[test]
    fn an_unreported_battery_state_stops_with_the_refusal_of_the_phone() {
        let mut replies = ready_phone();
        let _yes = replies.remove("battery-soc-ok");
        let run = check(&replies);
        assert_eq!(
            run.result.unwrap_err(),
            PhoneCheckFailure::BatteryUnreported(unreported(
                Variable::BatterySocOk,
                Some("unknown variable")
            ))
        );
    }

    #[test]
    fn an_empty_current_slot_stops() {
        let mut replies = ready_phone();
        let _slot = replies.insert("current-slot", "OKAY");
        let run = check(&replies);
        assert_eq!(
            run.result.unwrap_err(),
            PhoneCheckFailure::SlotUnreported(unreported(Variable::CurrentSlot, None))
        );
    }

    #[test]
    fn a_partition_smaller_than_its_image_stops_and_is_named_alone() {
        let mut replies = ready_phone();
        let _size = replies.insert("partition-size:boot_a", "OKAY0x10");
        let run = check(&replies);
        assert_eq!(
            run.result.unwrap_err(),
            PhoneCheckFailure::PartitionsUnfit {
                unfit: vec![UnfitPartition {
                    partition: "boot_a".to_owned(),
                    image_size: 100,
                    partition_size: 0x10,
                }],
                count: 2,
            }
        );
        assert!(
            !run.sent
                .iter()
                .any(|command| command == "getvar:max-download-size")
        );
    }

    #[test]
    fn an_unreported_partition_size_stops_with_the_refusal_of_the_phone() {
        let mut replies = ready_phone();
        let _size = replies.remove("partition-size:boot_a");
        let run = check(&replies);
        assert_eq!(
            run.result.unwrap_err(),
            PhoneCheckFailure::PartitionSizeUnreported(unreported(
                Variable::PartitionSize("boot_a".to_owned()),
                Some("unknown variable")
            ))
        );
        assert_eq!(run.sent.last().unwrap(), "getvar:partition-size:boot_a");
    }

    #[test]
    fn another_product_stops_before_the_other_requirements_are_read() {
        let mut replies = ready_phone();
        let _product = replies.insert("product", "OKAYOther");
        let _baseband = replies.insert("version-baseband", "OKAY2.0");
        let run = check(&replies);
        assert_eq!(
            run.result.unwrap_err(),
            PhoneCheckFailure::OtherProduct {
                product: "Other".to_owned()
            }
        );
        assert_eq!(run.sent, ["getvar:product"]);
    }
}
