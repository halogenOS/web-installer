// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

#![forbid(unsafe_code)]

mod error;
mod files;
mod operation;
mod package_files;
mod partition;
#[cfg(test)]
mod tests;

use std::io::BufReader;
use std::rc::Rc;

use byte_io::{ByteSource, SliceSource, SourceCursor, SourceError, clamp_len, to_len};
use prost::Message;
use update_metadata::{DeltaArchiveManifest, FULL_PAYLOAD_MINOR_VERSION, PayloadHeader};
use zip::result::ZipError;
use zip::{CompressionMethod, ZipArchive};
use zip_records::LONGEST_END_RECORD_LEN;

pub use error::{CopyFileError, PackageError};
pub use files::PartitionFiles;
pub use package_files::PackageFiles;

use crate::partition::PartitionImage;

const PAYLOAD_ENTRY: &str = "payload.bin";

pub(crate) struct StockPackage {
    payload: Rc<dyn ByteSource>,
    data_start: u64,
    block_size: u64,
    manifest: DeltaArchiveManifest,
}

impl StockPackage {
    pub(crate) fn open(package: Rc<dyn ByteSource>) -> Result<Self, PackageError> {
        let payload: Rc<dyn ByteSource> = Rc::new(find_payload(package)?);
        let mut header = [0; PayloadHeader::LEN];
        let header_len = to_len(header.len());
        payload
            .read_at(0, &mut header)
            .map_err(PackageError::Read)?;
        let PayloadHeader {
            manifest_len,
            metadata_signature_len,
        } = PayloadHeader::parse(&header).map_err(PackageError::Header)?;
        let manifest_bytes =
            read_range(&payload, header_len, manifest_len).map_err(PackageError::Read)?;
        let manifest = DeltaArchiveManifest::decode(manifest_bytes.as_slice())
            .map_err(PackageError::Manifest)?;
        let minor_version = manifest.minor_version();
        if minor_version != FULL_PAYLOAD_MINOR_VERSION {
            return Err(PackageError::Incremental { minor_version });
        }
        let block_size = u64::from(manifest.block_size());
        if block_size == 0 {
            return Err(PackageError::ZeroBlockSize);
        }
        let data_start = header_len
            .saturating_add(manifest_len)
            .saturating_add(u64::from(metadata_signature_len));
        Ok(Self {
            payload,
            data_start,
            block_size,
            manifest,
        })
    }

    pub(crate) fn partition_files(&self, partition: &str) -> Result<PartitionFiles, PackageError> {
        let image = self.partition(partition)?;
        let piece_len = image.largest_operation_len();
        PartitionFiles::open(partition, Rc::new(image), piece_len)
    }

    fn partition(&self, partition: &str) -> Result<PartitionImage, PackageError> {
        let update = self
            .manifest
            .partitions
            .iter()
            .find(|update| update.partition_name == partition)
            .ok_or_else(|| PackageError::PartitionMissing {
                partition: partition.to_owned(),
            })?;
        PartitionImage::new(
            Rc::clone(&self.payload),
            self.data_start,
            self.block_size,
            update,
        )
    }
}

fn find_payload(package: Rc<dyn ByteSource>) -> Result<SliceSource, PackageError> {
    let with_tail = PackageWithTail::read(Rc::clone(&package)).map_err(PackageError::Read)?;
    let input = BufReader::with_capacity(
        LONGEST_END_RECORD_LEN,
        SourceCursor::new(Rc::new(with_tail)),
    );
    let mut archive = ZipArchive::new(input).map_err(PackageError::Zip)?;
    let entry = archive
        .by_name_raw(PAYLOAD_ENTRY.as_bytes())
        .map_err(|error| match error {
            ZipError::FileNotFound => PackageError::PayloadMissing,
            error => PackageError::Zip(error),
        })?;
    if entry.compression() != CompressionMethod::Stored {
        return Err(PackageError::PayloadCompressed);
    }
    let start = entry
        .data_start()
        .ok_or(PackageError::Zip(ZipError::InvalidArchive(
            "the archive gives no start of the data of payload.bin".into(),
        )))?;
    let len = entry.size();
    drop(entry);
    SliceSource::new(package, start, len).map_err(PackageError::Read)
}

struct PackageWithTail {
    package: Rc<dyn ByteSource>,
    tail_start: u64,
    tail: Vec<u8>,
}

impl PackageWithTail {
    fn read(package: Rc<dyn ByteSource>) -> Result<Self, SourceError> {
        let tail_start = package.len().saturating_sub(to_len(LONGEST_END_RECORD_LEN));
        let tail = read_range(&package, tail_start, package.len() - tail_start)?;
        Ok(Self {
            package,
            tail_start,
            tail,
        })
    }

    fn tail_from(&self, offset: u64) -> Option<&[u8]> {
        let start = usize::try_from(offset.checked_sub(self.tail_start)?).ok()?;
        self.tail.get(start..)
    }
}

impl ByteSource for PackageWithTail {
    fn len(&self) -> u64 {
        self.package.len()
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), SourceError> {
        self.ensure_range(offset, to_len(buf.len()))?;
        match self.tail_from(offset) {
            Some(tail) => buf.copy_from_slice(&tail[..buf.len()]),
            None => self.package.read_at(offset, buf)?,
        }
        Ok(())
    }

    fn read_up_to(&self, offset: u64, len: u64) -> Result<Vec<u8>, SourceError> {
        self.ensure_range(offset, len)?;
        match self.tail_from(offset) {
            Some(tail) => Ok(tail[..clamp_len(len, tail.len())].to_vec()),
            None => self.package.read_up_to(offset, len),
        }
    }
}

fn read_range(source: &Rc<dyn ByteSource>, start: u64, len: u64) -> Result<Vec<u8>, SourceError> {
    let bytes = source.read_up_to(start, len)?;
    let delivered = to_len(bytes.len());
    if delivered < len {
        return Err(SourceError::BodyEndedEarly {
            declared: len,
            delivered,
        });
    }
    Ok(bytes)
}
