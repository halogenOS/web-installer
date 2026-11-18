// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::iter;
use std::rc::Rc;

use byte_io::{ByteSource, SourceError};
use release::{ImagePath, PartitionName, StockFirmware, StockPackage};
use stock_package::{CopyFileError, PackageError, PackageFiles};
use wasm_bindgen::JsValue;

use crate::host_sink::HostSink;
use crate::host_source::HostSource;
use crate::image_archive_files::{StockArchive, open_image_archive, read_archive_files};
use crate::page::Storage;
use crate::picked_files::{AskedFiles, PickedFileCheck, pick_files};
use crate::progress::Progress;
use crate::release_files_failure::{PackageReading, ReleaseFilesError};
use crate::release_images::Destination;
use crate::retry::{TransferFailure, pause_before_retry_millis};

pub(crate) enum OpenedFirmware {
    OtaPackage(OpenedPackage),
    ImageArchive(StockArchive),
}

pub(crate) async fn open_stock_firmware(
    storage: &Storage,
    firmware: &StockFirmware,
) -> Result<OpenedFirmware, ReleaseFilesError> {
    Ok(match firmware {
        StockFirmware::OtaPackage(package) => {
            OpenedFirmware::OtaPackage(open_stock_package(storage, package).await?)
        }
        StockFirmware::ImageArchive { parts } => {
            OpenedFirmware::ImageArchive(open_image_archive(storage, parts).await?)
        }
    })
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
            PickedFileCheck::Length,
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
    iter::successors(Some(error as &(dyn Error + 'static)), |&error| {
        error.source()
    })
    .find_map(|error| match error.downcast_ref::<SourceError>()? {
        SourceError::Transfer { source, .. } => source.get_ref()?.downcast_ref(),
        _ => None,
    })
}
