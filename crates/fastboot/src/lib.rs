// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

#![forbid(unsafe_code)]

mod error;
mod install;
mod link;
mod protocol;
mod sparse;

pub use error::FastbootError;
pub use install::{
    InstallHost, InstallProgress, PhoneCheck, UnmetRequirement, check_phone, install, install_steps,
};
pub use link::Link;
pub use protocol::Fastboot;
