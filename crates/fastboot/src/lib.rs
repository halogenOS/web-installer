// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

#![forbid(unsafe_code)]

mod checked_image;
mod error;
mod flash_plan;
mod install;
mod link;
mod phone_check;
mod protocol;
#[cfg(test)]
mod test_images;
mod variable;

pub use error::FastbootError;
pub use flash_plan::install_steps;
pub use install::{InstallHost, InstallProgress, install};
pub use link::Link;
pub use phone_check::{
    CheckedPhone, PassedCheck, PhoneCheck, PhoneCheckFailure, UnfitPartition, UnmetRequirement,
    check_phone,
};
pub use protocol::Fastboot;
pub use variable::{Unreported, Variable};
