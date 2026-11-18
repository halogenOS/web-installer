// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::io::Write;
use std::rc::Rc;

use byte_io::{ByteSource, READ_PIECE_LEN, SourceCursor};
use image_archive::{ArchiveError, Decoded, ImageArchive};
use release::{PartitionName, StockPackage, StockPackageAddress};
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

pub(crate) struct StockArchive {
    pub(crate) archive: ImageArchive,
    pub(crate) parts: Vec<StockPart>,
}

pub(crate) struct StockPart {
    pub(crate) bytes: Rc<dyn ByteSource>,
    pub(crate) download: Option<PartDownload>,
}

pub(crate) struct PartDownload {
    pub(crate) address: StockPackageAddress,
    pub(crate) stored: String,
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
    parts: &[StockPackage],
) -> Result<StockArchive, ReleaseFilesError> {
    let mut opened: BTreeMap<usize, (StockPart, Rc<dyn ByteSource>)> = BTreeMap::new();
    for (index, part) in parts.iter().enumerate() {
        if opened.contains_key(&index) {
            continue;
        }
        if storage.can_read_by_range(part.address.as_str()).await? {
            opened.extend([(index, open_ranged_part(storage, index, part).await?)]);
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
                PickedFileCheck::Sha256,
            )
            .await?;
            opened.extend(indexes.into_iter().zip(picked.into_iter().map(|bytes| {
                let part = StockPart {
                    bytes: Rc::clone(&bytes),
                    download: None,
                };
                (part, bytes)
            })));
        }
    }
    storage.report(&Progress::StockFirmwareChecked);
    let (parts, header_sources): (Vec<StockPart>, Vec<Rc<dyn ByteSource>>) =
        opened.into_values().unzip();
    let archive = ImageArchive::open(header_sources.into_iter().map(SourceCursor::new).collect())
        .map_err(|error| ReleaseFilesError::ImageArchive(Box::new(error)))?;
    Ok(StockArchive { archive, parts })
}

async fn open_ranged_part(
    storage: &Storage,
    index: usize,
    part: &StockPackage,
) -> Result<(StockPart, Rc<dyn ByteSource>), ReleaseFilesError> {
    let stored = StoredName::StockPart(index).to_string();
    storage.create_file(&stored, part.size).await?;
    let remote = HostSource::open(storage.remote_file(part.address.as_str(), part.size)?).map_err(
        |source| ReleaseFilesError::ImageArchive(Box::new(ArchiveError::Read(source.into()))),
    )?;
    let ranged = StockPart {
        bytes: Rc::new(open_stored(storage, &stored)?),
        download: Some(PartDownload {
            address: part.address.clone(),
            stored,
        }),
    };
    Ok((ranged, Rc::new(remote)))
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
        let image_name = StoredName::StockImage(partition).to_string();
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

async fn unpack_image(
    storage: &Storage,
    archive: &StockArchive,
    partition: &PartitionName,
    name: &str,
    built: &PartitionName,
) -> Result<HostSource, ReleaseFilesError> {
    let len = archive
        .archive
        .image_len(partition.as_str())
        .map_err(|error| ReleaseFilesError::ImageArchive(Box::new(error)))?;
    storage.create_file(name, len).await?;
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
    let mut sink = HostSink::open(storage.file(name)?)?;
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
