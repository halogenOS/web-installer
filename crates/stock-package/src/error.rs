// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::{fmt, io};

use byte_io::SourceError;
use fs_read::FsReadError;
use prost::DecodeError;
use sha256::Sha256Digest;
use update_metadata::HeaderError;
use zip::result::ZipError;

#[derive(Debug)]
pub enum PackageError {
    Zip(ZipError),
    PayloadMissing,
    PayloadCompressed,
    Read(SourceError),
    Header(HeaderError),
    Manifest(DecodeError),
    ZeroBlockSize,
    Incremental {
        minor_version: u32,
    },
    PartitionMissing {
        partition: String,
    },
    PartitionSizeMissing {
        partition: String,
    },
    UnsupportedOperation {
        partition: String,
        operation: i32,
    },
    OperationHashMissing {
        partition: String,
    },
    ExtentOutOfRange {
        partition: String,
        start_block: u64,
        num_blocks: u64,
    },
    DataOutOfRange {
        partition: String,
        offset: u64,
        length: u64,
    },
    Filesystem {
        partition: String,
        source: FsReadError,
    },
    FileMissing {
        partition: String,
        path: String,
    },
    FileRead {
        partition: String,
        path: String,
        source: SourceError,
    },
    FileMismatch {
        partition: String,
        path: String,
        expected: Sha256Digest,
        found: Sha256Digest,
    },
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zip(source) => write!(f, "the package is not a zip archive: {source}"),
            Self::PayloadMissing => {
                f.write_str("the zip archive of the package does not contain payload.bin")
            }
            Self::PayloadCompressed => {
                f.write_str("the package stores payload.bin compressed, not as it is")
            }
            Self::Read(source) => write!(f, "the payload could not be read: {source}"),
            Self::Header(source) => write!(f, "payload.bin is not a payload: {source}"),
            Self::Manifest(source) => write!(f, "the manifest of the payload is invalid: {source}"),
            Self::ZeroBlockSize => f.write_str("the manifest gives a block size of zero"),
            Self::Incremental { minor_version } => write!(
                f,
                "the payload updates an earlier build (minor version {minor_version}), so it is \
                 not a full package"
            ),
            Self::PartitionMissing { partition } => {
                write!(f, "the payload has no partition {partition}")
            }
            Self::PartitionSizeMissing { partition } => {
                write!(f, "the payload does not give the size of {partition}")
            }
            Self::UnsupportedOperation {
                partition,
                operation,
            } => write!(
                f,
                "an operation of {partition} has type {operation}, which the reader does not \
                 decode"
            ),
            Self::OperationHashMissing { partition } => write!(
                f,
                "the manifest does not give the SHA-256 of the data of an operation of \
                 {partition}"
            ),
            Self::ExtentOutOfRange {
                partition,
                start_block,
                num_blocks,
            } => write!(
                f,
                "an operation of {partition} writes {num_blocks} blocks from block {start_block}, \
                 outside the partition or over another operation"
            ),
            Self::DataOutOfRange {
                partition,
                offset,
                length,
            } => write!(
                f,
                "an operation of {partition} has {length} bytes of data at {offset}, outside the \
                 payload"
            ),
            Self::Filesystem { partition, source } => {
                write!(
                    f,
                    "the filesystem of {partition} could not be read: {source}"
                )
            }
            Self::FileMissing { partition, path } => {
                write!(f, "{partition} has no file {path}")
            }
            Self::FileRead {
                partition,
                path,
                source,
            } => write!(f, "{path} of {partition} could not be read: {source}"),
            Self::FileMismatch {
                partition,
                path,
                expected,
                found,
            } => write!(
                f,
                "{path} of {partition} has the SHA-256 {found}, but {expected} was expected"
            ),
        }
    }
}

impl Error for PackageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Zip(source) => Some(source),
            Self::Read(source) | Self::FileRead { source, .. } => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Header(source) => Some(source),
            Self::Filesystem { source, .. } => Some(source),
            Self::PayloadMissing
            | Self::PayloadCompressed
            | Self::ZeroBlockSize
            | Self::Incremental { .. }
            | Self::PartitionMissing { .. }
            | Self::PartitionSizeMissing { .. }
            | Self::UnsupportedOperation { .. }
            | Self::OperationHashMissing { .. }
            | Self::ExtentOutOfRange { .. }
            | Self::DataOutOfRange { .. }
            | Self::FileMissing { .. }
            | Self::FileMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CopyFileError {
    Package(Box<PackageError>),
    Sink(io::Error),
}

impl From<PackageError> for CopyFileError {
    fn from(source: PackageError) -> Self {
        Self::Package(Box::new(source))
    }
}

impl fmt::Display for CopyFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(source) => source.fmt(f),
            Self::Sink(source) => write!(f, "the file could not be written: {source}"),
        }
    }
}

impl Error for CopyFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Package(source) => Some(source),
            Self::Sink(source) => Some(source),
        }
    }
}
