// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::fmt;

use byte_io::SourceError;
use release::PartitionName;
use sparse_image::SparseImageError;

use crate::UnfitPartition;

#[derive(Debug)]
pub enum FastbootError<E> {
    Link(E),
    Failed {
        command: String,
        message: String,
    },
    UnexpectedReply {
        command: String,
        reply: String,
    },
    Read(SourceError),
    SparseImage {
        partition: PartitionName,
        source: SparseImageError,
    },
    PartitionUnfit(UnfitPartition),
    CannotSend {
        len: u64,
        max_download: u64,
    },
    NotFastbootd {
        is_userspace: String,
    },
}

impl<E: fmt::Display> fmt::Display for FastbootError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Link(source) => write!(f, "the connection to the phone failed: {source}"),
            Self::Failed { command, message } => {
                write!(f, "{command} failed on the phone: {message}")
            }
            Self::UnexpectedReply { command, reply } => {
                write!(f, "the phone sent {reply:?} after {command}")
            }
            Self::Read(source) => write!(f, "an image cannot be read: {source}"),
            Self::SparseImage { partition, source } => {
                write!(
                    f,
                    "the sparse image of {partition} cannot be read: {source}"
                )
            }
            Self::PartitionUnfit(UnfitPartition {
                partition,
                image_size,
                partition_size,
            }) => write!(
                f,
                "the image of {partition} writes {image_size} bytes, more than the \
                 {partition_size} bytes of the partition"
            ),
            Self::CannotSend { len, max_download } => write!(
                f,
                "an image of {len} bytes cannot be sent in downloads of at most {max_download} \
                 bytes"
            ),
            Self::NotFastbootd { is_userspace } => write!(
                f,
                "the phone did not restart into fastbootd: is-userspace is {is_userspace:?}"
            ),
        }
    }
}

impl<E: Error + 'static> Error for FastbootError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Link(source) => Some(source),
            Self::Read(source) => Some(source),
            Self::SparseImage { source, .. } => Some(source),
            Self::Failed { .. }
            | Self::PartitionUnfit(_)
            | Self::UnexpectedReply { .. }
            | Self::CannotSend { .. }
            | Self::NotFastbootd { .. } => None,
        }
    }
}
