// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;
use std::io;
use std::num::NonZeroUsize;
use std::ops::{Deref, Range};

use js_sys::{Promise, Uint8Array};
use release::ChunkOperationKind;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::carrier::Carrier;
use crate::file_digest::FileDigest;
use crate::host::{HostFailure, page_number};
use crate::host_link::PageLink;
use crate::number::integer_of;
use crate::progress::Progress;
use crate::range_arrival::RangeArrival;
use crate::retry::TransferFailure;

#[wasm_bindgen]
extern "C" {
    #[derive(Clone)]
    pub type ReleaseSite;

    #[wasm_bindgen(method, catch, js_name = fetchReleases)]
    fn fetch_releases(this: &ReleaseSite) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = fetchReleaseManifest)]
    fn fetch_release_manifest(this: &ReleaseSite, release: u64) -> Result<Promise, JsValue>;

    #[wasm_bindgen(extends = ReleaseSite)]
    #[derive(Clone)]
    pub type StoragePage;

    #[wasm_bindgen(method, catch, js_name = keepFiles)]
    fn keep_files(this: &StoragePage, names: Vec<String>) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = createFile)]
    fn create_file(this: &StoragePage, name: &str, size: JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn download(
        this: &StoragePage,
        address: &str,
        name: &str,
        offset: JsValue,
        size: JsValue,
        digest: FileDigest,
    ) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = downloadRange)]
    fn download_range(
        this: &StoragePage,
        address: &str,
        name: &str,
        offset: JsValue,
        length: JsValue,
        arrival: RangeArrival,
    ) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn file(this: &StoragePage, name: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch, js_name = fileFrom)]
    fn file_from(this: &StoragePage, name: &str, offset: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch, js_name = canReadByRange)]
    fn can_read_by_range(this: &StoragePage, address: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = canDownload)]
    fn can_download(this: &StoragePage, address: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = remoteFile)]
    fn remote_file(this: &StoragePage, address: &str, size: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch, js_name = waitBeforeRetry)]
    fn wait_before_retry(
        this: &StoragePage,
        failed_attempts: u32,
        milliseconds: f64,
    ) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = pickedFiles)]
    fn picked_files(this: &StoragePage) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = removeFile)]
    fn remove_file(this: &StoragePage, name: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = removeAllFiles)]
    fn remove_all_files(this: &StoragePage) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = freeStorage)]
    fn free_storage(this: &StoragePage) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method)]
    fn report(this: &StoragePage, progress: JsValue);

    #[wasm_bindgen(extends = StoragePage, extends = ReleaseSite)]
    #[derive(Clone)]
    pub type InstallPage;

    #[wasm_bindgen(method, catch, js_name = openPhone)]
    fn open_phone(this: &InstallPage) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = reopenPhone)]
    fn reopen_phone(this: &InstallPage) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, js_name = showPhoneMessage)]
    fn show_phone_message(this: &InstallPage, text: &str);

    #[wasm_bindgen(extends = StoragePage, extends = ReleaseSite)]
    #[derive(Clone)]
    pub type PackagePage;

    #[wasm_bindgen(method, catch, js_name = writePackage)]
    fn write_package(this: &PackagePage, bytes: &Uint8Array) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = closePackage)]
    fn close_package(this: &PackagePage) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, js_name = packageInPageStorage)]
    fn package_in_page_storage(this: &PackagePage) -> JsValue;

    #[wasm_bindgen(method, js_name = chunkCompressors)]
    fn chunk_compressors(this: &PackagePage) -> JsValue;

    #[wasm_bindgen(method, catch, js_name = compressChunk)]
    fn compress_chunk(
        this: &PackagePage,
        kind: JsValue,
        chunk: &Uint8Array,
    ) -> Result<Promise, JsValue>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PhoneProblem {
    NoWebUsb,
    NotFound,
    Several,
    NotAccessible,
    NoFastbootInterface,
    InterfaceBusy,
}

impl fmt::Display for PhoneProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoWebUsb => "this browser does not let the page use USB",
            Self::NotFound => "the phone is not connected",
            Self::Several => "several connected devices look like the chosen phone",
            Self::NotAccessible => "the system does not let the browser open the phone",
            Self::NoFastbootInterface => "the chosen device is not a phone in fastboot mode",
            Self::InterfaceBusy => "another program uses the fastboot interface of the phone",
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum PhoneOpening {
    Opened(#[serde(with = "serde_wasm_bindgen::preserve")] JsValue),
    Unavailable(PhoneProblem),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Transfer<T> {
    Received { value: T },
    Failed { failure: TransferFailure },
}

#[derive(Deserialize)]
pub(crate) struct PickedFile {
    pub(crate) name: String,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    pub(crate) file: JsValue,
}

#[derive(Deserialize)]
pub(crate) struct KeptFile {
    pub(crate) name: String,
    pub(crate) size: u64,
}

#[derive(Debug)]
pub(crate) enum OpenFailure {
    Unavailable(PhoneProblem),
    Page(HostFailure),
}

#[derive(Clone)]
pub(crate) struct Page {
    page: InstallPage,
    storage: Storage,
}

impl Page {
    pub(crate) fn new(page: InstallPage) -> Self {
        Self {
            storage: Storage(page.clone().into()),
            page,
        }
    }

    pub(crate) async fn open_phone(&self) -> Result<PageLink, OpenFailure> {
        Self::link_of(self.page.open_phone()).await
    }

    pub(crate) async fn reopen_phone(&self) -> Result<PageLink, OpenFailure> {
        Self::link_of(self.page.reopen_phone()).await
    }

    async fn link_of(called: Result<Promise, JsValue>) -> Result<PageLink, OpenFailure> {
        match settle(called).await.map_err(OpenFailure::Page)? {
            PhoneOpening::Opened(link) => PageLink::open(link).map_err(OpenFailure::Page),
            PhoneOpening::Unavailable(problem) => Err(OpenFailure::Unavailable(problem)),
        }
    }

    pub(crate) fn show_phone_message(&self, text: &str) {
        self.page.show_phone_message(text);
    }
}

impl Deref for Page {
    type Target = Storage;

    fn deref(&self) -> &Storage {
        &self.storage
    }
}

#[derive(Clone)]
pub(crate) struct Storage(StoragePage);

impl Storage {
    pub(crate) fn new(page: StoragePage) -> Self {
        Self(page)
    }

    pub(crate) fn site(&self) -> &ReleaseSite {
        &self.0
    }

    pub(crate) async fn keep_files(
        &self,
        names: Vec<String>,
    ) -> Result<Vec<KeptFile>, HostFailure> {
        settle(self.0.keep_files(names)).await
    }

    pub(crate) async fn create_file(&self, name: &str, size: u64) -> Result<(), HostFailure> {
        let _created: JsValue = settle_value(self.0.create_file(name, page_number(size)?)).await?;
        Ok(())
    }

    pub(crate) fn start_download(
        &self,
        address: &str,
        name: &str,
        offset: u64,
        size: u64,
        digest: &FileDigest,
    ) -> Result<Download, HostFailure> {
        let promise = self.0.download(
            address,
            name,
            page_number(offset)?,
            page_number(size)?,
            digest.share(),
        )?;
        Ok(Download(JsFuture::from(promise)))
    }

    pub(crate) fn start_range_download(
        &self,
        address: &str,
        name: &str,
        range: &Range<u64>,
        arrival: &RangeArrival,
    ) -> Result<Download, HostFailure> {
        let promise = self.0.download_range(
            address,
            name,
            page_number(range.start)?,
            page_number(range.end - range.start)?,
            arrival.share(),
        )?;
        Ok(Download(JsFuture::from(promise)))
    }

    pub(crate) fn file(&self, name: &str) -> Result<JsValue, HostFailure> {
        Ok(self.0.file(name)?)
    }

    pub(crate) fn file_from(&self, name: &str, offset: u64) -> Result<JsValue, HostFailure> {
        Ok(self.0.file_from(name, page_number(offset)?)?)
    }

    pub(crate) async fn can_read_by_range(&self, address: &str) -> Result<bool, HostFailure> {
        settle(self.0.can_read_by_range(address)).await
    }

    pub(crate) async fn can_download(&self, address: &str) -> Result<bool, HostFailure> {
        settle(self.0.can_download(address)).await
    }

    pub(crate) fn remote_file(&self, address: &str, size: u64) -> Result<JsValue, HostFailure> {
        Ok(self.0.remote_file(address, page_number(size)?)?)
    }

    pub(crate) async fn wait_before_retry(
        &self,
        failed_attempts: u32,
        milliseconds: f64,
    ) -> Result<(), HostFailure> {
        let _waited: JsValue =
            settle_value(self.0.wait_before_retry(failed_attempts, milliseconds)).await?;
        Ok(())
    }

    pub(crate) async fn picked_files(&self) -> Result<Vec<PickedFile>, HostFailure> {
        settle(self.0.picked_files()).await
    }

    pub(crate) async fn remove_file(&self, name: &str) -> Result<(), HostFailure> {
        let _removed: JsValue = settle_value(self.0.remove_file(name)).await?;
        Ok(())
    }

    pub(crate) async fn remove_all_files(&self) -> Result<(), HostFailure> {
        let _removed: JsValue = settle_value(self.0.remove_all_files()).await?;
        Ok(())
    }

    pub(crate) async fn free_storage(&self) -> Result<u64, HostFailure> {
        let free = settle_value(self.0.free_storage()).await?;
        let number = free
            .as_f64()
            .ok_or_else(|| HostFailure::contract_violation("freeStorage must give a number"))?;
        Ok(integer_of(number)?)
    }

    pub(crate) fn report(&self, progress: &Progress<'_>) {
        let value =
            serde_wasm_bindgen::to_value(progress).expect("every number of a report is below 2^53");
        self.0.report(value);
    }
}

pub(crate) struct PackageFile {
    page: PackagePage,
    carrier: Carrier,
}

impl PackageFile {
    pub(crate) fn new(page: PackagePage) -> Self {
        Self {
            page,
            carrier: Carrier::new(),
        }
    }

    pub(crate) async fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let view = self.carrier.carry(bytes)?;
        let _written: JsValue = settle_value(self.page.write_package(&view))
            .await
            .map_err(io::Error::from)?;
        Ok(())
    }

    pub(crate) async fn close(self) -> Result<(), HostFailure> {
        let _closed: JsValue = settle_value(self.page.close_package()).await?;
        Ok(())
    }
}

pub(crate) fn package_in_page_storage(page: &PackagePage) -> Result<bool, HostFailure> {
    page.package_in_page_storage()
        .as_bool()
        .ok_or_else(|| HostFailure::contract_violation("packageInPageStorage must give a boolean"))
}

pub(crate) struct ChunkCompressors(PackagePage);

impl ChunkCompressors {
    pub(crate) fn new(page: PackagePage) -> Self {
        Self(page)
    }

    pub(crate) fn count(&self) -> Result<NonZeroUsize, HostFailure> {
        let not_a_count =
            || HostFailure::contract_violation("chunkCompressors must give a whole number above 0");
        let number = self
            .0
            .chunk_compressors()
            .as_f64()
            .ok_or_else(not_a_count)?;
        usize::try_from(integer_of(number)?)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(not_a_count)
    }

    pub(crate) fn start(
        &self,
        kind: ChunkOperationKind,
        chunk: &[u8],
    ) -> Result<Compression, HostFailure> {
        let kind =
            serde_wasm_bindgen::to_value(&kind).expect("serde writes a unit variant as text");
        let promise = self.0.compress_chunk(kind, &Uint8Array::from(chunk))?;
        Ok(Compression(JsFuture::from(promise)))
    }
}

pub(crate) struct Compression(JsFuture);

impl Compression {
    pub(crate) async fn data(self) -> Result<Vec<u8>, HostFailure> {
        self.0
            .await?
            .dyn_into::<Uint8Array>()
            .map(|data| data.to_vec())
            .map_err(|_| HostFailure::contract_violation("compressChunk must give a Uint8Array"))
    }
}

pub(crate) async fn fetch_releases(site: &ReleaseSite) -> Result<Transfer<String>, HostFailure> {
    settle(site.fetch_releases()).await
}

pub(crate) async fn fetch_release_manifest(
    site: &ReleaseSite,
    release: u64,
) -> Result<Transfer<String>, HostFailure> {
    settle(site.fetch_release_manifest(release)).await
}

pub(crate) struct Download(JsFuture);

impl Download {
    pub(crate) async fn end(self) -> Result<Transfer<u64>, HostFailure> {
        read_value(self.0.await?)
    }
}

async fn settle_value(called: Result<Promise, JsValue>) -> Result<JsValue, HostFailure> {
    Ok(JsFuture::from(called?).await?)
}

async fn settle<T: DeserializeOwned>(called: Result<Promise, JsValue>) -> Result<T, HostFailure> {
    read_value(settle_value(called).await?)
}

fn read_value<T: DeserializeOwned>(value: JsValue) -> Result<T, HostFailure> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|error| HostFailure::contract_violation(error.to_string()))
}
