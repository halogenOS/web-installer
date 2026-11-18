// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::{fmt, io, iter};

use byte_io::SourceError;
use image_archive::ArchiveError;
use image_build::ImageBuildError;
use release::{PartitionName, ReleaseError, ReleaseFile};
use sha256::Sha256Digest;
use stock_package::PackageError;

use crate::archive_ranges::RangeEndedEarly;
use crate::host::HostFailure;
use crate::page_failure::{PageFailure, REPORT_DEFECT};
use crate::remote_file::RemoteFile;
use crate::retry::TransferFailure;

const BYTES_PER_MEGABYTE: u64 = 1_000_000;

pub(crate) const FREE_STORAGE: &str =
    "Free space on this computer, or leave private browsing if you are using it, then try again.";

const RELOAD: &str =
    "Reload this page and try again. Report the message above to us if it happens a second time.";

#[derive(Debug)]
pub(crate) enum ReleaseFilesError {
    Page(HostFailure),
    ReleasesFetch {
        failure: TransferFailure,
    },
    ReleasesUnreadable {
        source: serde_json::Error,
    },
    ReleaseMissing,
    AssetMissing {
        release: String,
        name: String,
    },
    Release {
        release: String,
        source: ReleaseError,
    },
    StorageShort(StorageShortage),
    Fetch {
        file: RemoteFile,
        failure: TransferFailure,
    },
    StockPackage {
        reading: PackageReading,
        source: Box<PackageError>,
    },
    PickedRead {
        name: String,
        source: SourceError,
    },
    ImageArchive(Box<ArchiveError>),
    RangeEndedEarly(RangeEndedEarly),
    StockImageFile(Box<PackageError>),
    FileDiffers(Box<FileDifference>),
    Stored {
        name: String,
        source: SourceError,
    },
    StoreWrite {
        name: String,
        source: io::Error,
    },
    Build {
        partition: PartitionName,
        source: ImageBuildError,
    },
    ImageDiffers {
        partition: PartitionName,
        listed: Sha256Digest,
        found: Sha256Digest,
    },
    FileNotInStore {
        partition: PartitionName,
        sha256: Sha256Digest,
    },
}

#[derive(Debug)]
pub(crate) struct FileDifference {
    pub(crate) file: RemoteFile,
    pub(crate) listed: Sha256Digest,
    pub(crate) listed_size: u64,
    pub(crate) found: Sha256Digest,
    pub(crate) found_size: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum PackageReading {
    Address,
    PickedFile,
}

impl PackageReading {
    pub(crate) fn failure(self, source: PackageError) -> ReleaseFilesError {
        ReleaseFilesError::StockPackage {
            reading: self,
            source: Box::new(source),
        }
    }

    fn next_step(self, error: &PackageError) -> &'static str {
        match (self, is_unreadable(error)) {
            (Self::Address, true) => RELOAD,
            (Self::Address, false) => "Report this text to us.",
            (Self::PickedFile, true) => {
                "Choose the same stock package on your next try. No new download is needed."
            }
            (Self::PickedFile, false) => {
                "On your next try, download the stock package again from \
                 the link and choose the new file."
            }
        }
    }
}

fn is_unreadable(error: &(dyn Error + 'static)) -> bool {
    iter::successors(Some(error), |&error| error.source()).any(|error| {
        matches!(
            error.downcast_ref::<SourceError>(),
            Some(
                SourceError::Io(_)
                    | SourceError::Host { .. }
                    | SourceError::Transfer { .. }
                    | SourceError::BodyChanged
            )
        )
    })
}

pub(crate) fn wrong_length_failure(name: &str, len: u64) -> PageFailure {
    PageFailure::new(
        format_args!(
            "The chosen file {name} is none of the files above. It has {len} bytes, and none of \
             them has that size."
        ),
        "Choose the files you downloaded from the links above.",
    )
}

pub(crate) fn wrong_digest_failure(name: &str, found: Sha256Digest) -> PageFailure {
    PageFailure::new(
        format_args!(
            "The chosen file {name} is none of the files above. Its SHA-256 is {found}, and none \
             of them has it."
        ),
        "Download the file again from its link above and choose the new file.",
    )
}

impl ReleaseFilesError {
    pub(crate) fn next_step(&self) -> &'static str {
        match self {
            Self::Page(failure) if failure.is_storage_full() => FREE_STORAGE,
            Self::StoreWrite { source, .. } if source.kind() == io::ErrorKind::QuotaExceeded => {
                FREE_STORAGE
            }
            Self::StorageShort(_) => FREE_STORAGE,
            Self::Page(_)
            | Self::Stored { .. }
            | Self::StoreWrite { .. }
            | Self::ReleasesFetch { .. }
            | Self::RangeEndedEarly(_) => RELOAD,
            Self::ReleaseMissing => "Reload the page, then choose a release again.",
            Self::StockPackage { reading, source } => reading.next_step(source),
            Self::PickedRead { .. } => "Choose the same file on your next try.",
            Self::ImageArchive(source) if matches!(**source, ArchiveError::Read(_)) => RELOAD,
            Self::StockImageFile(source) if is_unreadable(&**source) => RELOAD,
            Self::Build { .. } | Self::FileNotInStore { .. } => REPORT_DEFECT,
            Self::Fetch { .. }
            | Self::ImageArchive(_)
            | Self::StockImageFile(_)
            | Self::FileDiffers(_)
            | Self::ReleasesUnreadable { .. }
            | Self::AssetMissing { .. }
            | Self::Release { .. }
            | Self::ImageDiffers { .. } => "Report this text to us.",
        }
    }
}

fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_MEGABYTE)
}

#[derive(Debug)]
pub(crate) struct StorageShortage {
    pub(crate) needed: u64,
    pub(crate) free: u64,
    pub(crate) package_counted: bool,
}

impl fmt::Display for StorageShortage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.package_counted {
            return write!(
                f,
                "The install needs {} MB of storage in this browser, counting the zip, \
                 and the browser gives it {} MB",
                megabytes(self.needed),
                megabytes(self.free)
            );
        }
        write!(
            f,
            "The install needs {} MB of storage in this browser, and the browser gives it {} MB",
            megabytes(self.needed),
            megabytes(self.free)
        )
    }
}

impl fmt::Display for ReleaseFilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page(failure) => write!(f, "The browser failed: {failure}"),
            Self::ReleasesFetch { failure } => {
                write!(f, "The list of releases cannot be fetched: {failure}")
            }
            Self::ReleasesUnreadable { source } => {
                write!(f, "The server sent no list of releases: {source}")
            }
            Self::ReleaseMissing => {
                f.write_str("The list of releases no longer includes the chosen release")
            }
            Self::AssetMissing { release, name } => write!(
                f,
                "The release {release} on GitHub does not contain the file {name}"
            ),
            Self::Release { release, source } => write!(
                f,
                "{} of the release {release} cannot be read: {source}",
                ReleaseFile::Manifest.name()
            ),
            Self::StorageShort(shortage) => shortage.fmt(f),
            Self::Fetch {
                file: RemoteFile::Source { source, path },
                failure: TransferFailure::Status { status: 404, .. },
            } => write!(
                f,
                "{path} is missing at {} {}",
                source.repository, source.commit
            ),
            Self::Fetch { file, failure } => write!(f, "{file} cannot be fetched: {failure}"),
            Self::StockPackage { source, .. } => {
                write!(f, "The installer cannot read the stock package: {source}")
            }
            Self::PickedRead { name, source } => {
                write!(f, "The chosen file {name} cannot be read: {source}")
            }
            Self::ImageArchive(source) => write!(
                f,
                "The installer cannot read the image archive of the stock firmware: {source}"
            ),
            Self::RangeEndedEarly(ended) => write!(f, "{ended}"),
            Self::StockImageFile(source) => {
                write!(f, "The installer cannot read a stock file: {source}")
            }
            Self::FileDiffers(difference) => {
                let FileDifference {
                    file,
                    listed,
                    listed_size,
                    found,
                    found_size,
                } = &**difference;
                write!(
                    f,
                    "{file} differs from the release: the release lists {listed_size} bytes with \
                     SHA-256 {listed}. The file has {found_size} bytes with SHA-256 {found}"
                )
            }
            Self::Stored { name, source } => {
                write!(f, "The stored file {name} cannot be read: {source}")
            }
            Self::StoreWrite { name, source } => {
                write!(f, "The stored file {name} cannot be written: {source}")
            }
            Self::Build { partition, source } => {
                write!(f, "The image of {partition} cannot be built: {source}")
            }
            Self::ImageDiffers {
                partition,
                listed,
                found,
            } => write!(
                f,
                "The built image of {partition} differs from the release: the release lists \
                 SHA-256 {listed}, and the image has SHA-256 {found}"
            ),
            Self::FileNotInStore { partition, sha256 } => write!(
                f,
                "The image of {partition} is built from the file with SHA-256 {sha256}, which \
                 the stored files of the image lack"
            ),
        }
    }
}

impl Error for ReleaseFilesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Page(source) => Some(source),
            Self::Release { source, .. } => Some(source),
            Self::ReleasesUnreadable { source } => Some(source),
            Self::Stored { source, .. } | Self::PickedRead { source, .. } => Some(source),
            Self::StoreWrite { source, .. } => Some(source),
            Self::StockPackage { source, .. } | Self::StockImageFile(source) => Some(source),
            Self::ImageArchive(source) => Some(source),
            Self::Build { source, .. } => Some(source),
            Self::StorageShort(_)
            | Self::Fetch { .. }
            | Self::RangeEndedEarly(_)
            | Self::ReleasesFetch { .. }
            | Self::ReleaseMissing
            | Self::AssetMissing { .. }
            | Self::FileDiffers(_)
            | Self::ImageDiffers { .. }
            | Self::FileNotInStore { .. } => None,
        }
    }
}

impl From<HostFailure> for ReleaseFilesError {
    fn from(failure: HostFailure) -> Self {
        Self::Page(failure)
    }
}
