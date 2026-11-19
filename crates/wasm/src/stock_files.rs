// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::iter;
use std::rc::Rc;

use byte_io::{ByteSource, SourceError};
use release::{ImagePath, PartitionName, Source, SourceIndex, StockPackage};
use stock_package::{CopyFileError, PackageError, PackageFiles};
use wasm_bindgen::JsValue;

use crate::host_sink::HostSink;
use crate::host_source::HostSource;
use crate::image_archive_files::{
    PartToRead, StockArchive, StockPart, open_image_archive, read_archive_files,
};
use crate::page::Storage;
use crate::picked_files::{AskedFiles, PickedFileCheck, pick_files};
use crate::progress::Progress;
use crate::release_files_failure::{PackageReading, ReleaseFilesError};
use crate::release_images::Destination;
use crate::retry::{TransferFailure, pause_before_retry_millis};

pub(crate) enum OpenedFirmware<Part = StockPart> {
    OtaPackage(OpenedPackage),
    ImageArchive(StockArchive<Part>),
}

pub(crate) async fn open_stock_firmware(
    storage: &Storage,
    index: SourceIndex,
    source: &Source,
) -> Result<OpenedFirmware<PartToRead>, ReleaseFilesError> {
    Ok(match source {
        Source::OtaPackage(package) => {
            OpenedFirmware::OtaPackage(open_stock_package(storage, package).await?)
        }
        Source::ImageArchive { parts } => {
            OpenedFirmware::ImageArchive(open_image_archive(storage, index, parts).await?)
        }
        Source::Git(_) => {
            unreachable!("Release::new accepts stock files and images only from firmware sources")
        }
    })
}

impl OpenedFirmware<PartToRead> {
    /// The page reads an OTA package in place and stores the downloaded parts of an image archive.
    pub(crate) fn stored_bytes(&self) -> u64 {
        match self {
            Self::OtaPackage(_) => 0,
            Self::ImageArchive(archive) => archive.stored_part_bytes(),
        }
    }

    pub(crate) async fn store_parts(
        self,
        storage: &Storage,
    ) -> Result<OpenedFirmware, ReleaseFilesError> {
        Ok(match self {
            Self::OtaPackage(package) => OpenedFirmware::OtaPackage(package),
            Self::ImageArchive(archive) => {
                OpenedFirmware::ImageArchive(archive.store_parts(storage).await?)
            }
        })
    }
}

impl<Part> OpenedFirmware<Part> {
    /// The bytes that the page stores while it reads stock files of the partition. It unpacks the
    /// whole partition image of an image archive and reads an OTA package in place.
    pub(crate) fn unpacked_len(&self, partition: &PartitionName) -> Result<u64, ReleaseFilesError> {
        match self {
            Self::OtaPackage(_) => Ok(0),
            Self::ImageArchive(archive) => archive.image_len(partition),
        }
    }
}

impl OpenedFirmware {
    pub(crate) async fn read_files(
        &mut self,
        storage: &Storage,
        partition: &PartitionName,
        stock_files: &[StockFile<'_>],
    ) -> Result<(), ReleaseFilesError> {
        let total = stock_files.iter().map(|file| file.to.size).sum();
        let progress = |done| Progress::ReadingStockFiles {
            partition: partition.as_str(),
            done,
            total,
        };
        match self {
            Self::OtaPackage(package) => {
                read_package_files(storage, package, stock_files, progress).await
            }
            Self::ImageArchive(archive) => {
                read_archive_files(storage, archive, partition, stock_files, progress).await
            }
        }
    }
}

pub(crate) struct OpenedPackage {
    files: PackageFiles,
    reading: PackageReading,
}

async fn open_stock_package(
    storage: &Storage,
    package: &StockPackage,
) -> Result<OpenedPackage, ReleaseFilesError> {
    let address = package.address.as_str();
    let (file, reading): (Rc<dyn ByteSource>, _) = if storage.can_read_by_range(address).await? {
        let reading = PackageReading::Address;
        let file = storage.remote_file(address, package.size)?;
        (Rc::new(open_package_file(file, reading)?), reading)
    } else {
        let mut picked = pick_files(
            storage,
            AskedFiles::StockFirmware,
            &[package.into()],
            PickedFileCheck::Identity,
        )
        .await?;
        let file = picked
            .pop()
            .expect("pick_files gives one source for each awaited file");
        (file, PackageReading::PickedFile)
    };
    let files = PackageFiles::open(file).map_err(|source| reading.failure(source))?;
    Ok(OpenedPackage { files, reading })
}

fn open_package_file(
    file: JsValue,
    reading: PackageReading,
) -> Result<HostSource, ReleaseFilesError> {
    HostSource::open(file).map_err(|source| reading.failure(PackageError::Read(source)))
}

pub(crate) struct StockFile<'a> {
    pub(crate) partition: &'a PartitionName,
    pub(crate) path: &'a ImagePath,
    pub(crate) to: Destination<'a>,
}

async fn read_package_files<'a>(
    storage: &Storage,
    package: &mut OpenedPackage,
    stock_files: &[StockFile<'_>],
    progress: impl Fn(u64) -> Progress<'a>,
) -> Result<(), ReleaseFilesError> {
    let reading = package.reading;
    let mut done = 0;
    storage.report(&progress(done));
    for file in stock_files {
        let mut failed_attempts = 0;
        done += loop {
            let name = file.to.name.to_string();
            let mut sink = HostSink::open(storage.file_from(&name, file.to.offset)?)?;
            let error = match package.files.copy_file(
                file.partition.as_str(),
                file.path.as_str(),
                file.to.sha256,
                &mut sink,
            ) {
                Ok(len) => break len,
                Err(CopyFileError::Sink(source)) => {
                    return Err(ReleaseFilesError::StoreWrite { name, source });
                }
                Err(CopyFileError::Package(error)) => error,
            };
            failed_attempts += 1;
            let pause = failed_request(&error)
                .and_then(|failure| pause_before_retry_millis(failure, failed_attempts));
            let Some(milliseconds) = pause else {
                return Err(reading.failure(*error));
            };
            storage
                .wait_before_retry(failed_attempts, milliseconds)
                .await?;
        };
        storage.report(&progress(done));
    }
    package.files.close_partitions();
    Ok(())
}

fn failed_request(error: &PackageError) -> Option<&TransferFailure> {
    let outermost: &(dyn Error + 'static) = error;
    let mut error_chain = iter::successors(Some(outermost), |&error| error.source());
    error_chain.find_map(|error| match error.downcast_ref::<SourceError>()? {
        SourceError::Transfer { source, .. } => source.get_ref()?.downcast_ref(),
        _ => None,
    })
}
