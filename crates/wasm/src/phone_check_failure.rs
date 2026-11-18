// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;

use fastboot::{PhoneCheckFailure, UnfitPartition, Unreported};
use release::ReleaseName;

pub(crate) const RESTART_INTO_FASTBOOT: &str =
    "Restart the phone into fastboot mode, then run the install again.";

const UNREPORTED_VARIABLE: &str = "The phone did not report a value that the install checks:";
const LOW_BATTERY: &str = "The phone reports that its battery is too low for the install.";
const LOW_BATTERY_NEXT_STEP: &str = "Charge the phone, then run the install again.";
const PARTITIONS_UNFIT: &str = "Some images of the release are larger than their partitions \
    on this phone. Each partition shows the image size, then the partition size, in bytes:";
const TRANSFER_SIZE_TOO_SMALL: &str =
    "The phone accepts too few bytes in one transfer to send the images:";
pub(crate) const PHONE_CANNOT_TAKE_RELEASE: &str =
    "Report this text to us: this release does not install on this phone over USB.";

#[derive(Debug)]
pub(crate) struct PhoneUnfit {
    pub(crate) device: ReleaseName,
    pub(crate) failure: PhoneCheckFailure,
}

impl PhoneUnfit {
    pub(crate) fn next_step(&self) -> &'static str {
        match &self.failure {
            PhoneCheckFailure::ProductUnreported(_)
            | PhoneCheckFailure::UnlockStateUnreported(_)
            | PhoneCheckFailure::BatteryUnreported(_)
            | PhoneCheckFailure::SlotUnreported(_)
            | PhoneCheckFailure::PartitionSizeUnreported(_)
            | PhoneCheckFailure::TransferSizeUnreported(_) => RESTART_INTO_FASTBOOT,
            PhoneCheckFailure::OtherProduct { .. } => {
                "Choose the release for the connected phone, or connect the phone that you chose \
                 the release for, then run the install again."
            }
            PhoneCheckFailure::RequirementsUnmet { .. } => {
                "Update the firmware of the phone to a version that the release lists, then run \
                 the install again."
            }
            PhoneCheckFailure::BootloaderLocked => {
                "Unlock the bootloader with `fastboot flashing unlock` on a computer, then run the \
                 install again. Unlocking erases all data on the phone."
            }
            PhoneCheckFailure::BatteryLow => LOW_BATTERY_NEXT_STEP,
            PhoneCheckFailure::PartitionsUnfit { .. }
            | PhoneCheckFailure::TransferSizeTooSmall { .. } => PHONE_CANNOT_TAKE_RELEASE,
        }
    }
}

impl fmt::Display for PhoneUnfit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            PhoneCheckFailure::ProductUnreported(unreported)
            | PhoneCheckFailure::UnlockStateUnreported(unreported)
            | PhoneCheckFailure::BatteryUnreported(unreported)
            | PhoneCheckFailure::SlotUnreported(unreported)
            | PhoneCheckFailure::PartitionSizeUnreported(unreported)
            | PhoneCheckFailure::TransferSizeUnreported(unreported) => {
                write!(f, "{UNREPORTED_VARIABLE} ")?;
                write_unreported(f, unreported)?;
                f.write_str(".")
            }
            PhoneCheckFailure::OtherProduct { product } => write!(
                f,
                "The connected phone reports the product {product}, and the release for {} does \
                 not install on that product.",
                self.device
            ),
            PhoneCheckFailure::RequirementsUnmet { unmet, .. } => {
                f.write_str("The phone does not meet the requirements of the release:")?;
                for requirement in unmet {
                    write!(
                        f,
                        " {} must be one of {}, and the phone reports {}.",
                        requirement.variable.getvar_name(),
                        requirement.values.join(", "),
                        requirement.reported.as_deref().unwrap_or("nothing")
                    )?;
                }
                Ok(())
            }
            PhoneCheckFailure::BootloaderLocked => {
                f.write_str("The bootloader of the phone is locked.")
            }
            PhoneCheckFailure::BatteryLow => f.write_str(LOW_BATTERY),
            PhoneCheckFailure::PartitionsUnfit { unfit, .. } => {
                f.write_str(PARTITIONS_UNFIT)?;
                unfit.iter().try_for_each(
                    |UnfitPartition {
                         partition,
                         image_size,
                         partition_size,
                     }| {
                        write!(f, " {partition}: {image_size}, {partition_size}.")
                    },
                )
            }
            PhoneCheckFailure::TransferSizeTooSmall { download_limit } => {
                write!(f, "{TRANSFER_SIZE_TOO_SMALL} {download_limit}.")
            }
        }
    }
}

fn write_unreported(f: &mut fmt::Formatter<'_>, unreported: &Unreported) -> fmt::Result {
    write!(f, "getvar:{}", unreported.variable)?;
    match &unreported.refusal {
        Some(message) => write!(f, " ({message})"),
        None => Ok(()),
    }
}
