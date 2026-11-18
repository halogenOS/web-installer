// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use fastboot::{PassedCheck, PhoneCheck, PhoneCheckFailure};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PhoneCheckReport<'a> {
    verdict: CheckVerdict,
    reading: CheckReading<'a>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckVerdict {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Answer {
    Yes,
    No,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "check",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum CheckReading<'a> {
    Model { product: Option<&'a str> },
    Bootloader { unlocked: Option<Answer> },
    Battery { soc_ok: Option<Answer> },
    Slot { current_slot: Option<&'a str> },
    Partitions { fit: Option<PartitionFit> },
    TransferSize { download_limit: Option<u32> },
}

#[derive(Debug, Serialize)]
struct PartitionFit {
    fitting: usize,
    count: usize,
}

impl<'a> PhoneCheckReport<'a> {
    pub(crate) fn of(check: PhoneCheck<'a>) -> Self {
        match check {
            Ok(passed) => Self {
                verdict: CheckVerdict::Passed,
                reading: CheckReading::of_passed(passed),
            },
            Err(failure) => Self {
                verdict: CheckVerdict::Failed,
                reading: CheckReading::of_failure(failure),
            },
        }
    }
}

impl<'a> CheckReading<'a> {
    fn of_passed(check: &'a PassedCheck) -> Self {
        match check {
            PassedCheck::Model { product } => Self::Model {
                product: Some(product),
            },
            PassedCheck::Bootloader => Self::Bootloader {
                unlocked: Some(Answer::Yes),
            },
            PassedCheck::Battery => Self::Battery {
                soc_ok: Some(Answer::Yes),
            },
            PassedCheck::Slot { slot } => Self::Slot {
                current_slot: Some(slot),
            },
            PassedCheck::Partitions { count } => Self::Partitions {
                fit: Some(PartitionFit {
                    fitting: *count,
                    count: *count,
                }),
            },
            PassedCheck::TransferSize { download_limit } => Self::TransferSize {
                download_limit: Some(*download_limit),
            },
        }
    }

    fn of_failure(failure: &'a PhoneCheckFailure) -> Self {
        match failure {
            PhoneCheckFailure::ProductUnreported(_) => Self::Model { product: None },
            PhoneCheckFailure::OtherProduct { product }
            | PhoneCheckFailure::RequirementsUnmet { product, .. } => Self::Model {
                product: Some(product),
            },
            PhoneCheckFailure::UnlockStateUnreported(_) => Self::Bootloader { unlocked: None },
            PhoneCheckFailure::BootloaderLocked => Self::Bootloader {
                unlocked: Some(Answer::No),
            },
            PhoneCheckFailure::BatteryUnreported(_) => Self::Battery { soc_ok: None },
            PhoneCheckFailure::BatteryLow => Self::Battery {
                soc_ok: Some(Answer::No),
            },
            PhoneCheckFailure::SlotUnreported(_) => Self::Slot { current_slot: None },
            PhoneCheckFailure::PartitionSizeUnreported(_) => Self::Partitions { fit: None },
            PhoneCheckFailure::PartitionsUnfit { unfit, count } => Self::Partitions {
                fit: Some(PartitionFit {
                    fitting: count - unfit.len(),
                    count: *count,
                }),
            },
            PhoneCheckFailure::TransferSizeUnreported(_) => Self::TransferSize {
                download_limit: None,
            },
            PhoneCheckFailure::TransferSizeTooSmall { download_limit } => Self::TransferSize {
                download_limit: Some(*download_limit),
            },
        }
    }
}
