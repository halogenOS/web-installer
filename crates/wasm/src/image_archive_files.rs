// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::io::Write;
use std::rc::Rc;

use byte_io::{ByteSource, READ_PIECE_LEN, SourceCursor};
use image_archive::{ArchiveError, Decoded, ImageArchive};
use release::{PartitionName, SourceIndex, StockPackage, StockPackageAddress};
use stock_package::{CopyFileError, PartitionFiles};

use crate::archive_ranges::extract_from_ranges;
use crate::host_sink::HostSink;
use crate::host_source::HostSource;
use crate::page::Storage;
use crate::picked_files::{AskedFile, AskedFiles, PickedFileCheck, pick_files};
use crate::progress::{Progress, ShownPercent};
use crate::release_files_failure::ReleaseFilesError;
use crate::release_images::StoredName;
use crate::stock_files::StockFile;

pub(crate) struct StockArchive<Part = StockPart> {
    pub(crate) source: SourceIndex,
    pub(crate) archive: ImageArchive,
    pub(crate) parts: Vec<Part>,
}

pub(crate) struct StockPart {
    pub(crate) bytes: Rc<dyn ByteSource>,
    pub(crate) download: Option<PartDownload>,
}

pub(crate) struct PartDownload {
    pub(crate) address: StockPackageAddress,
    pub(crate) stored: String,
}

/// A part of an opened image archive, before the page stores the parts it reads by range, so
/// the storage check of the run can come first.
pub(crate) enum PartToRead {
    Picked(Rc<dyn ByteSource>),
    ByRange(StockPackage),
}

impl PartToRead {
    /// The page checks a picked part when the person picks it, and a part read by range image by
    /// image, as the bytes of each image arrive.
    fn is_checked(&self) -> bool {
        matches!(self, Self::Picked(_))
    }
}

impl<Part> StockArchive<Part> {
    pub(crate) fn image_len(&self, partition: &PartitionName) -> Result<u64, ReleaseFilesError> {
        self.archive
            .image_len(partition.as_str())
            .map_err(|error| ReleaseFilesError::ImageArchive(Box::new(error)))
    }
}

impl StockArchive<PartToRead> {
    /// The page stores each part it reads by range at full length and reads a picked part in place.
    pub(crate) fn stored_part_bytes(&self) -> u64 {
        self.parts
            .iter()
            .map(|part| match part {
                PartToRead::Picked(_) => 0,
                PartToRead::ByRange(package) => package.size,
            })
            .sum()
    }

    pub(crate) async fn store_parts(
        self,
        storage: &Storage,
    ) -> Result<StockArchive, ReleaseFilesError> {
        let Self {
            source,
            archive,
            parts: to_read,
        } = self;
        let mut parts = Vec::with_capacity(to_read.len());
        for (index, part) in to_read.into_iter().enumerate() {
            parts.push(match part {
                PartToRead::Picked(bytes) => StockPart {
                    bytes,
                    download: None,
                },
                PartToRead::ByRange(package) => {
                    store_ranged_part(storage, source, index, package).await?
                }
            });
        }
        Ok(StockArchive {
            source,
            archive,
            parts,
        })
    }
}

impl StockArchive {
    pub(crate) fn readers(&self) -> Vec<SourceCursor> {
        self.parts
            .iter()
            .map(|part| SourceCursor::new(Rc::clone(&part.bytes)))
            .collect()
    }
}

pub(crate) async fn open_image_archive(
    storage: &Storage,
    source: SourceIndex,
    parts: &[StockPackage],
) -> Result<StockArchive<PartToRead>, ReleaseFilesError> {
    let mut opened: BTreeMap<usize, (PartToRead, Rc<dyn ByteSource>)> = BTreeMap::new();
    for (index, part) in parts.iter().enumerate() {
        if opened.contains_key(&index) {
            continue;
        }
        if storage.can_read_by_range(part.address.as_str()).await? {
            opened.extend([(index, open_ranged_part(storage, part)?)]);
        } else {
            let host = part.address.host();
            let (indexes, awaited): (Vec<usize>, Vec<AskedFile<'_>>) = parts
                .iter()
                .enumerate()
                .skip(index)
                .filter(|(_, later)| later.address.host() == host)
                .map(|(index, later)| (index, AskedFile::from(later)))
                .unzip();
            let picked = pick_files(
                storage,
                AskedFiles::StockFirmware,
                &awaited,
                PickedFileCheck::Identity,
            )
            .await?;
            opened.extend(
                indexes.into_iter().zip(
                    picked
                        .into_iter()
                        .map(|bytes| (PartToRead::Picked(Rc::clone(&bytes)), bytes)),
                ),
            );
        }
    }
    let (parts, header_sources): (Vec<PartToRead>, Vec<Rc<dyn ByteSource>>) =
        opened.into_values().unzip();
    if parts.iter().all(PartToRead::is_checked) {
        storage.report(&Progress::StockFirmwareChecked);
    }
    let archive = ImageArchive::open(header_sources.into_iter().map(SourceCursor::new).collect())
        .map_err(|error| ReleaseFilesError::ImageArchive(Box::new(error)))?;
    Ok(StockArchive {
        source,
        archive,
        parts,
    })
}

fn open_ranged_part(
    storage: &Storage,
    part: &StockPackage,
) -> Result<(PartToRead, Rc<dyn ByteSource>), ReleaseFilesError> {
    let remote = HostSource::open(storage.remote_file(part.address.as_str(), part.size)?).map_err(
        |source| ReleaseFilesError::ImageArchive(Box::new(ArchiveError::Read(source.into()))),
    )?;
    Ok((PartToRead::ByRange(part.clone()), Rc::new(remote)))
}

async fn store_ranged_part(
    storage: &Storage,
    source: SourceIndex,
    index: usize,
    part: StockPackage,
) -> Result<StockPart, ReleaseFilesError> {
    let stored = StoredName::StockPart {
        source,
        part: index,
    }
    .to_string();
    storage.create_file(&stored, part.size).await?;
    Ok(StockPart {
        bytes: Rc::new(open_stored(storage, &stored)?),
        download: Some(PartDownload {
            address: part.address,
            stored,
        }),
    })
}

pub(crate) async fn read_archive_files<'a>(
    storage: &Storage,
    archive: &StockArchive,
    built: &PartitionName,
    stock_files: &[StockFile<'_>],
    progress: impl Fn(u64) -> Progress<'a>,
) -> Result<(), ReleaseFilesError> {
    let mut by_partition: BTreeMap<&PartitionName, Vec<&StockFile<'_>>> = BTreeMap::new();
    for file in stock_files {
        by_partition.entry(file.partition).or_default().push(file);
    }
    let mut done = 0;
    for (partition, files) in by_partition {
        let image_name = StoredName::StockImage {
            source: archive.source,
            partition,
        }
        .to_string();
        let image = unpack_image(storage, archive, partition, &image_name, built).await?;
        storage.report(&progress(done));
        let mut image_files =
            PartitionFiles::open(partition.as_str(), Rc::new(image), READ_PIECE_LEN)
                .map_err(|error| ReleaseFilesError::StockImageFile(Box::new(error)))?;
        for file in files {
            let name = file.to.name.to_string();
            let mut sink = HostSink::open(storage.file_from(&name, file.to.offset)?)?;
            done += image_files
                .copy_file(file.path.as_str(), file.to.sha256, &mut sink)
                .map_err(|error| match error {
                    CopyFileError::Sink(source) => ReleaseFilesError::StoreWrite { name, source },
                    CopyFileError::Package(error) => ReleaseFilesError::StockImageFile(error),
                })?;
            storage.report(&progress(done));
        }
        drop(image_files);
        storage.remove_file(&image_name).await?;
    }
    Ok(())
}

pub(crate) async fn unpack_image(
    storage: &Storage,
    archive: &StockArchive,
    partition: &PartitionName,
    name: &str,
    built: &PartitionName,
) -> Result<HostSource, ReleaseFilesError> {
    storage
        .create_file(name, archive.image_len(partition)?)
        .await?;
    if archive.parts.iter().all(|part| part.download.is_none()) {
        write_image(storage, name, built, |sink, progress| {
            archive
                .archive
                .extract_image(archive.readers(), partition.as_str(), sink, progress)
        })?;
    } else {
        extract_from_ranges(storage, archive, partition, name, built).await?;
    }
    open_stored(storage, name)
}

pub(crate) fn write_image(
    storage: &Storage,
    name: &str,
    built: &PartitionName,
    extraction: impl FnOnce(&mut dyn Write, &mut dyn FnMut(Decoded)) -> Result<u64, ArchiveError>,
) -> Result<(), ReleaseFilesError> {
    let store_write = |source| ReleaseFilesError::StoreWrite {
        name: name.to_owned(),
        source,
    };
    let mut sink = HostSink::open(storage.file_from(name, 0)?)?;
    let mut shown = ShownPercent::default();
    let _written: u64 = extraction(&mut sink, &mut |decoded| {
        if shown.advances_to(decoded.done, decoded.total) {
            storage.report(&Progress::UnpackingStockImage {
                partition: built.as_str(),
                done: decoded.done,
                total: decoded.total,
            });
        }
    })
    .map_err(|error| match error {
        ArchiveError::Write(source) => store_write(source),
        error => ReleaseFilesError::ImageArchive(Box::new(error)),
    })?;
    sink.flush().map_err(store_write)
}

fn open_stored(storage: &Storage, name: &str) -> Result<HostSource, ReleaseFilesError> {
    HostSource::open(storage.file(name)?).map_err(|source| ReleaseFilesError::Stored {
        name: name.to_owned(),
        source,
    })
}
