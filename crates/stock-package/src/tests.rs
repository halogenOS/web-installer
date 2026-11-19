// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::Cell;
use std::io::{Cursor, Write};
use std::rc::Rc;

use byte_io::{ByteSource, MemorySource, SourceError, to_len};
use fs_read::testing::erofs_compressed::{
    self, CompressedImage, FILE_NAME, Head, IndexKind, Stored,
};
use prost::Message;
use sha256::Sha256Digest;
use update_metadata::install_operation::Type;
use update_metadata::{
    DeltaArchiveManifest, Extent, FULL_PAYLOAD_MINOR_VERSION, InstallOperation, PartitionInfo,
    PartitionUpdate, PayloadHeader,
};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

use crate::partition::PartitionImage;
use crate::{CopyFileError, PAYLOAD_ENTRY, PackageError, PackageFiles};

const BLOCK: usize = 4096;

const SIGNATURE: &[u8] = b"metadata signature";

fn library_content() -> Vec<u8> {
    (0..3 * BLOCK + 100)
        .map(|n| u8::try_from(n % 251).unwrap())
        .collect()
}

fn erofs_image() -> Vec<u8> {
    let content = library_content();
    let mut image = CompressedImage::new(to_len(content.len()), IndexKind::Full);
    for (n, block) in content.chunks(BLOCK).enumerate() {
        image.extents.push(erofs_compressed::Extent {
            start: to_len(n * BLOCK),
            head: Head::Plain,
            stored: n,
        });
        image.stored.push(Stored::plain(block.to_vec(), 1));
    }
    image.build()
}

fn payload(image: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    let operations = image
        .chunks(BLOCK)
        .enumerate()
        .filter(|(_, bytes)| bytes.iter().any(|&byte| byte != 0))
        .map(|(block, bytes)| {
            let operation = InstallOperation {
                r#type: Type::Replace.into(),
                data_offset: Some(to_len(data.len())),
                data_length: Some(to_len(bytes.len())),
                dst_extents: vec![Extent {
                    start_block: Some(u64::try_from(block).unwrap()),
                    num_blocks: Some(1),
                }],
                data_sha256_hash: Some(Sha256Digest::of(bytes).as_bytes().to_vec()),
            };
            data.extend_from_slice(bytes);
            operation
        })
        .collect();
    let manifest = DeltaArchiveManifest {
        block_size: Some(u32::try_from(BLOCK).unwrap()),
        minor_version: Some(FULL_PAYLOAD_MINOR_VERSION),
        partitions: vec![PartitionUpdate {
            partition_name: "system".to_owned(),
            new_partition_info: Some(PartitionInfo {
                size: Some(to_len(image.len())),
                ..PartitionInfo::default()
            }),
            operations,
            ..PartitionUpdate::default()
        }],
        ..DeltaArchiveManifest::default()
    }
    .encode_to_vec();
    let mut payload = PayloadHeader {
        manifest_len: to_len(manifest.len()),
        metadata_signature_len: u32::try_from(SIGNATURE.len()).unwrap(),
    }
    .to_bytes()
    .to_vec();
    payload.extend_from_slice(&manifest);
    payload.extend_from_slice(SIGNATURE);
    payload.extend_from_slice(&data);
    payload
}

fn package() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file(PAYLOAD_ENTRY, stored).unwrap();
    writer.write_all(&payload(&erofs_image())).unwrap();
    writer.finish().unwrap().into_inner()
}

struct CountedReads {
    package: MemorySource,
    reads: Cell<u32>,
}

impl ByteSource for CountedReads {
    fn len(&self) -> u64 {
        self.package.len()
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), SourceError> {
        self.reads.set(self.reads.get() + 1);
        self.package.read_at(offset, buf)
    }

    fn read_up_to(&self, offset: u64, len: u64) -> Result<Vec<u8>, SourceError> {
        self.reads.set(self.reads.get() + 1);
        self.package.read_up_to(offset, len)
    }
}

#[test]
fn opening_reads_the_tail_of_the_zip_archive_once() {
    let package = Rc::new(CountedReads {
        package: MemorySource::new(package()),
        reads: Cell::new(0),
    });
    // Inside the call, `Rc::clone` would take `Rc<dyn ByteSource>` as its type and expect a
    // reference to one, so the second handle is bound first and coerced at the call.
    let package_source = Rc::clone(&package);
    let _opened = PackageFiles::open(package_source).unwrap();
    assert_eq!(package.reads.get(), 3);
}

#[test]
fn a_file_of_a_partition_is_copied_from_the_package() {
    let mut package = PackageFiles::open(Rc::new(MemorySource::new(package()))).unwrap();
    let content = library_content();
    let mut copied = Vec::new();

    let copied_len = package
        .copy_file("system", FILE_NAME, Sha256Digest::of(&content), &mut copied)
        .unwrap();

    assert_eq!(copied, content);
    assert_eq!(copied_len, to_len(content.len()));
}

#[test]
fn a_copied_file_whose_digest_differs_fails() {
    let mut package = PackageFiles::open(Rc::new(MemorySource::new(package()))).unwrap();

    let copied = package.copy_file(
        "system",
        FILE_NAME,
        Sha256Digest::of(b"other"),
        &mut Vec::new(),
    );

    assert!(matches!(
        copied,
        Err(CopyFileError::Package(error)) if matches!(*error, PackageError::FileMismatch { .. })
    ));
}

fn partition_of_one_block(data: &[u8]) -> PartitionImage {
    let update = PartitionUpdate {
        partition_name: "system".to_owned(),
        new_partition_info: Some(PartitionInfo {
            size: Some(to_len(BLOCK)),
            ..PartitionInfo::default()
        }),
        operations: vec![InstallOperation {
            r#type: Type::Replace.into(),
            data_offset: Some(0),
            data_length: Some(to_len(data.len())),
            dst_extents: vec![Extent {
                start_block: Some(0),
                num_blocks: Some(1),
            }],
            data_sha256_hash: Some(Sha256Digest::of(data).as_bytes().to_vec()),
        }],
        ..PartitionUpdate::default()
    };
    let payload = Rc::new(MemorySource::new(data.to_vec()));
    PartitionImage::new(payload, 0, to_len(BLOCK), &update).unwrap()
}

#[test]
fn a_block_that_its_operation_fills_in_part_ends_in_zeros() {
    let data = [7; 100];
    let mut block = [1; BLOCK];

    partition_of_one_block(&data)
        .read_at(0, &mut block)
        .unwrap();

    assert_eq!(block[..data.len()], data);
    assert!(block[data.len()..].iter().all(|&byte| byte == 0));
}

#[test]
fn a_read_past_the_data_of_an_operation_gives_zeros() {
    let mut tail = [1; 50];

    partition_of_one_block(&[7; 100])
        .read_at(200, &mut tail)
        .unwrap();

    assert_eq!(tail, [0; 50]);
}
