// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::fmt;

use failure_text::cause_sentence;
use fastboot::FastbootError;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host::HostFailure;
use crate::host_link::LinkFailure;
use crate::page::{OpenFailure, PhoneProblem};
use crate::page_failure::{REPORT_DEFECT, page_failure};
use crate::phone_check_failure::{PHONE_CANNOT_TAKE_RELEASE, PhoneUnfit, RESTART_INTO_FASTBOOT};
use crate::release_files_failure::{FREE_STORAGE, ReleaseFilesError};

const PHONE_UNCHANGED: &str = "Nothing was written to your phone";

#[derive(Debug)]
pub(crate) enum InstallError {
    Page(HostFailure),
    PhoneUnavailable(PhoneProblem),
    Phone(FastbootError<LinkFailure>),
    PhoneUnfit(PhoneUnfit),
    Files(ReleaseFilesError),
    FilesKept(HostFailure),
}

#[wasm_bindgen(js_name = phoneProblemFailure)]
#[must_use]
pub fn phone_problem_failure(problem: JsValue) -> JsValue {
    serde_wasm_bindgen::from_value(problem)
        .map_or_else(
            |error| InstallError::Page(HostFailure::contract_violation(error.to_string())),
            InstallError::PhoneUnavailable,
        )
        .to_page_failure()
}

impl InstallError {
    pub(crate) fn to_page_failure(&self) -> JsValue {
        page_failure(self, self.next_step())
    }

    fn next_step(&self) -> &'static str {
        match self {
            Self::Page(failure) if failure.is_storage_full() => FREE_STORAGE,
            Self::Page(_) => {
                "Reload the page, then run the install again. If this happens again, report this \
                 text to us."
            }
            Self::PhoneUnavailable(problem)
            | Self::Phone(FastbootError::Link(LinkFailure::Unavailable(problem))) => {
                problem.next_step()
            }
            Self::Phone(
                FastbootError::Link(LinkFailure::Contract(_)) | FastbootError::SparseImage { .. },
            ) => REPORT_DEFECT,
            Self::Phone(FastbootError::PartitionUnfit(_)) => PHONE_CANNOT_TAKE_RELEASE,
            Self::Phone(_) => {
                "Your phone is in fastboot mode. Keep it plugged in, then run the install again."
            }
            Self::PhoneUnfit(unfit) => unfit.next_step(),
            Self::Files(failure) => failure.next_step(),
            Self::FilesKept(_) => {
                "The next install deletes them. To delete them now, clear the data of this site \
                 in the settings of the browser."
            }
        }
    }
}

impl PhoneProblem {
    fn next_step(self) -> &'static str {
        match self {
            Self::NoWebUsb => "Open this page in a browser with WebUSB, such as Chromium.",
            Self::NotFound | Self::Several => {
                "Plug in only the phone, restart it into fastboot mode, then run the install again."
            }
            Self::NotAccessible => {
                "Let the browser use the phone: on Linux, add a udev rule for it, on Windows, \
                 install the WinUSB driver for it. Then run the install again."
            }
            Self::NoFastbootInterface => RESTART_INTO_FASTBOOT,
            Self::InterfaceBusy => {
                "Close every other program and tab that uses the phone, such as fastboot or adb, \
                 then run the install again."
            }
        }
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page(failure) => write!(f, "The browser failed: {failure}"),
            Self::PhoneUnavailable(problem) => write!(f, "The phone cannot be used: {problem}"),
            Self::Phone(FastbootError::Link(LinkFailure::Unavailable(problem))) => write!(
                f,
                "The phone cannot be used after its restart into fastbootd: {problem}"
            ),
            Self::Phone(source) => write!(f, "The install stopped, because {source}"),
            Self::PhoneUnfit(unfit) => write!(f, "{unfit} {PHONE_UNCHANGED}"),
            Self::Files(failure) => write!(f, "{} {PHONE_UNCHANGED}", cause_sentence(failure)),
            Self::FilesKept(failure) => write!(
                f,
                "The install is done and the phone restarts into the new system, but the browser \
                 did not delete the images that the install stored: {failure}"
            ),
        }
    }
}

impl Error for InstallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Page(source) | Self::FilesKept(source) => Some(source),
            Self::Phone(source) => Some(source),
            Self::Files(source) => Some(source),
            Self::PhoneUnavailable(_) | Self::PhoneUnfit(_) => None,
        }
    }
}

impl From<HostFailure> for InstallError {
    fn from(failure: HostFailure) -> Self {
        Self::Page(failure)
    }
}

impl From<ReleaseFilesError> for InstallError {
    fn from(failure: ReleaseFilesError) -> Self {
        Self::Files(failure)
    }
}

impl From<OpenFailure> for InstallError {
    fn from(failure: OpenFailure) -> Self {
        match failure {
            OpenFailure::Unavailable(problem) => Self::PhoneUnavailable(problem),
            OpenFailure::Page(failure) => Self::Page(failure),
        }
    }
}

impl From<FastbootError<LinkFailure>> for InstallError {
    fn from(error: FastbootError<LinkFailure>) -> Self {
        Self::Phone(error)
    }
}
