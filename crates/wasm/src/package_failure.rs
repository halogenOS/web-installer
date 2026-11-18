// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::{fmt, io};

use byte_io::SourceError;
use ota_package::PackageError;
use sha256::Sha256Digest;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host::HostFailure;
use crate::page_failure::{REPORT_DEFECT, page_failure};
use crate::release_files_failure::ReleaseFilesError;

const CHOOSE_OTHER_PLACE: &str =
    "Choose a place with free space for the zip, then download it again.";

#[wasm_bindgen(js_name = saveDialogFailure)]
#[must_use]
pub fn save_dialog_failure() -> JsValue {
    PackageFailure::SaveDialogFailed.to_page_failure()
}

#[derive(Debug)]
pub(crate) enum PackageFailure {
    Page(HostFailure),
    SaveDialogFailed,
    Files(ReleaseFilesError),
    Write(PackageError),
    ImagesUnreadable(SourceError),
    FileWrite(io::Error),
    FileClose(HostFailure),
    Differs {
        listed: Sha256Digest,
        listed_size: u64,
        found: Sha256Digest,
        found_size: u64,
    },
}

impl PackageFailure {
    pub(crate) fn to_page_failure(&self) -> JsValue {
        page_failure(self, self.next_step())
    }

    fn next_step(&self) -> &'static str {
        match self {
            Self::Files(failure) => failure.next_step(),
            Self::SaveDialogFailed => "Download the zip again.",
            Self::Page(_) | Self::Write(_) => REPORT_DEFECT,
            Self::ImagesUnreadable(_) => {
                "Reload the page, then download the zip again. If this happens again, report this \
                 text to us."
            }
            Self::FileWrite(_) | Self::FileClose(_) => CHOOSE_OTHER_PLACE,
            Self::Differs { .. } => "Report this text to us.",
        }
    }
}

impl fmt::Display for PackageFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page(failure) => write!(f, "The browser failed: {failure}"),
            Self::Files(failure) => failure.fmt(f),
            Self::SaveDialogFailed => f.write_str("The save dialog of the browser did not open"),
            Self::Write(source) => write!(f, "The zip cannot be written: {source}"),
            Self::ImagesUnreadable(source) => {
                write!(f, "The stored images cannot be read for the zip: {source}")
            }
            Self::FileWrite(source) => {
                write!(
                    f,
                    "The zip cannot be written into the chosen file: {source}"
                )
            }
            Self::FileClose(failure) => write!(
                f,
                "The browser did not save the zip into the chosen file: {failure}"
            ),
            Self::Differs {
                listed,
                listed_size,
                found,
                found_size,
            } => write!(
                f,
                "The zip differs from the release: the release lists {listed_size} bytes with \
                 SHA-256 {listed}, and the zip has {found_size} bytes with SHA-256 {found}. The \
                 chosen file is unchanged."
            ),
        }
    }
}

impl Error for PackageFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Write(source) => Some(source),
            Self::ImagesUnreadable(source) => Some(source),
            Self::FileWrite(source) => Some(source),
            Self::Page(source) | Self::FileClose(source) => Some(source),
            Self::Files(source) => Some(source),
            Self::SaveDialogFailed | Self::Differs { .. } => None,
        }
    }
}

impl From<ReleaseFilesError> for PackageFailure {
    fn from(failure: ReleaseFilesError) -> Self {
        Self::Files(failure)
    }
}
