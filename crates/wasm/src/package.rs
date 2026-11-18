// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::VecDeque;
use std::rc::Rc;

use ota_package::{OtaPackage, PackageError, PayloadChunk, PayloadImage, PayloadOperations};
use release::{Package, Release, payload_images};
use sha256::RunningDigest;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host::HostFailure;
use crate::host_source::HostSource;
use crate::package_failure::PackageFailure;
use crate::page::{ChunkCompressors, PackageFile, PackagePage, Storage, package_in_page_storage};
use crate::progress::{Progress, ReleasePlan, ShownPercent};
use crate::release_files_failure::ReleaseFilesError;
use crate::release_images::{ImagesFor, StoredName, store_checked_images};
use crate::releases::read_release_of_tag;

#[wasm_bindgen(js_name = savePackage)]
pub async fn save_package(page: PackagePage, release_tag: String) -> Result<(), JsValue> {
    run(&Storage::new(page.clone().into()), page, &release_tag)
        .await
        .map_err(|failure| failure.to_page_failure())
}

async fn run(
    storage: &Storage,
    page: PackagePage,
    release_tag: &str,
) -> Result<(), PackageFailure> {
    storage.report(&Progress::ReadingRelease);
    let (published, release) = read_release_of_tag(storage.site(), release_tag).await?;
    storage.report(&Progress::ReleaseRead(ReleasePlan::of(
        &release,
        ImagesFor::Package,
    )));
    let stored_package = package_in_page_storage(&page)
        .map_err(PackageFailure::Page)?
        .then_some(&release.parts().package);
    store_checked_images(
        storage,
        &release,
        &published,
        ImagesFor::Package,
        stored_package,
    )
    .await?;
    write_package(storage, &release, page).await?;
    let _removed_or_left_for_next_run: Result<(), HostFailure> = storage.remove_all_files().await;
    storage.report(&Progress::PackageSaved);
    Ok(())
}

async fn write_package(
    storage: &Storage,
    release: &Release,
    page: PackagePage,
) -> Result<(), PackageFailure> {
    let package = &release.parts().package;
    let operations =
        PayloadOperations::new(open_payload_images(storage, release)?, &package.partitions)
            .map_err(package_failure)?;
    let mut ota_package = OtaPackage::new(package, &operations).map_err(package_failure)?;
    let compressors = ChunkCompressors::new(page.clone());
    let mut out = PackageOutput {
        file: PackageFile::new(page),
        storage,
        package,
        digest: RunningDigest::default(),
        written: 0,
        shown: ShownPercent::default(),
    };
    out.write(ota_package.head()).await?;
    write_operation_data(&operations, &compressors, &mut ota_package, &mut out).await?;
    out.write(&ota_package.tail()).await?;
    out.close().await
}

async fn write_operation_data(
    operations: &PayloadOperations<'_>,
    compressors: &ChunkCompressors,
    ota_package: &mut OtaPackage<'_>,
    out: &mut PackageOutput<'_>,
) -> Result<(), PackageFailure> {
    let window = 2 * compressors.count().map_err(PackageFailure::Page)?.get();
    let mut chunks = operations.chunks();
    let mut compressions = VecDeque::with_capacity(window);
    loop {
        while compressions.len() < window
            && let Some(chunk) = chunks.next()
        {
            let PayloadChunk { listed, bytes } = chunk.map_err(package_failure)?;
            let compression = compressors
                .start(listed.kind(), &bytes)
                .map_err(PackageFailure::Page)?;
            compressions.push_back((listed, compression));
        }
        let Some((listed, compression)) = compressions.pop_front() else {
            return Ok(());
        };
        let data = compression.data().await.map_err(PackageFailure::Page)?;
        ota_package
            .take_data(listed, &data)
            .map_err(package_failure)?;
        out.write(&data).await?;
    }
}

fn package_failure(error: PackageError) -> PackageFailure {
    match error {
        PackageError::Read { source, .. } => PackageFailure::ImagesUnreadable(source),
        error => PackageFailure::Write(error),
    }
}

fn open_payload_images(
    storage: &Storage,
    release: &Release,
) -> Result<Vec<PayloadImage>, PackageFailure> {
    let parts = release.parts();
    payload_images(&parts.images, &parts.package.partitions)
        .map(|(partition, image)| {
            let name = StoredName::Image(partition).to_string();
            let source = storage
                .file(&name)
                .map_err(Into::into)
                .and_then(HostSource::open)
                .map_err(|source| ReleaseFilesError::Stored { name, source })?;
            Ok(PayloadImage {
                partition: partition.clone(),
                source: Rc::new(source),
                sha256: image.sha256(),
            })
        })
        .collect()
}

struct PackageOutput<'a> {
    file: PackageFile,
    storage: &'a Storage,
    package: &'a Package,
    digest: RunningDigest,
    written: u64,
    shown: ShownPercent,
}

impl PackageOutput<'_> {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), PackageFailure> {
        self.file
            .write(bytes)
            .await
            .map_err(PackageFailure::FileWrite)?;
        self.digest.update(bytes);
        self.written += bytes.len() as u64;
        let total = self.package.size;
        if self.shown.advances_to(self.written, total) {
            self.storage.report(&Progress::WritingPackage {
                done: self.written,
                total,
            });
        }
        Ok(())
    }

    async fn close(self) -> Result<(), PackageFailure> {
        let found = self.digest.finish();
        if found != self.package.sha256 || self.written != self.package.size {
            return Err(PackageFailure::Differs {
                listed: self.package.sha256,
                listed_size: self.package.size,
                found,
                found_size: self.written,
            });
        }
        self.file.close().await.map_err(PackageFailure::FileClose)
    }
}
