// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::io::Write;
use std::rc::Rc;

use byte_io::{Body, ByteSource, SliceSource, SourceBody};
use image_build::{FileToFill, ImageBuildError, build_image};
use release::{
    BuiltImage, BuiltImageFiles, BuiltImageStores, FileStore, Image, Origin, Package,
    PartitionName, Release, ReleaseFile, Step, StoredFile, WrittenImage, lfs_pointer,
    payload_images,
};
use sha256::Sha256Digest;

use crate::arrived_bytes::{ArrivedBytes, FileArrival};
use crate::file_digest::FileDigest;
use crate::host::HostFailure;
use crate::host_sink::HostSink;
use crate::host_source::HostSource;
use crate::page::{Download, Storage, Transfer};
use crate::picked_release_files::{
    PickedReleaseFiles, pick_release_files_the_page_cannot_download,
};
use crate::progress::{Progress, ShownPercent};
use crate::release_files_failure::{FileDifference, ReleaseFilesError, StorageShortage};
use crate::releases::PublishedRelease;
use crate::remote_file::RemoteFile;
use crate::stock_files::{OpenedFirmware, StockFile, open_stock_firmware};

#[derive(Clone, Copy, Debug)]
pub(crate) enum StoredName<'a> {
    Image(&'a PartitionName),
    OwnStore(&'a PartitionName),
    SharedStore,
    SuperEmpty,
    StockPart(usize),
    StockImage(&'a PartitionName),
}

impl Display for StoredName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(partition) => write!(f, "{partition}.img"),
            Self::OwnStore(partition) => write!(f, "{partition}.store"),
            Self::SharedStore => f.write_str("shared-files.store"),
            Self::SuperEmpty => f.write_str("super_empty.img"),
            Self::StockPart(index) => write!(f, "stock-part-{index}.7z"),
            Self::StockImage(partition) => write!(f, "stock-{partition}.img"),
        }
    }
}

pub(crate) struct Destination<'a> {
    pub(crate) name: StoredName<'a>,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) sha256: Sha256Digest,
}

#[derive(Clone, Copy)]
pub(crate) enum ImagesFor {
    Install,
    Package,
}

impl ImagesFor {
    pub(crate) fn images(self, release: &Release) -> Vec<(&PartitionName, &Image)> {
        let parts = release.parts();
        match self {
            Self::Install => parts
                .images
                .iter()
                .filter(|(partition, _)| {
                    parts.install.iter().any(|step| {
                        matches!(step, Step::Flash { partition: flashed, .. } if flashed == *partition)
                    })
                })
                .collect(),
            Self::Package => payload_images(&parts.images, &parts.package.partitions).collect(),
        }
    }

    pub(crate) fn steps(self, release: &Release) -> Vec<&Step> {
        match self {
            Self::Install => fastboot::install_steps(release),
            Self::Package => release.parts().install.iter().collect(),
        }
    }

    fn super_layouts(
        self,
        release: &Release,
        published: &PublishedRelease,
    ) -> Result<Vec<Fetch<'static>>, ReleaseFilesError> {
        match self {
            Self::Install => release
                .parts()
                .install
                .iter()
                .filter_map(|step| match step {
                    Step::UpdateSuper { sha256, size, .. } => {
                        Some(published.asset(ReleaseFile::SuperEmpty).map(|file| Fetch {
                            file,
                            to: Destination {
                                name: StoredName::SuperEmpty,
                                offset: 0,
                                size: *size,
                                sha256: *sha256,
                            },
                        }))
                    }
                    Step::Flash { .. } | Step::RebootFastboot | Step::Erase { .. } => None,
                })
                .collect(),
            Self::Package => Ok(Vec::new()),
        }
    }
}

pub(crate) async fn store_checked_images(
    storage: &Storage,
    release: &Release,
    published: &PublishedRelease,
    run: ImagesFor,
    stored_package: Option<&Package>,
) -> Result<(), ReleaseFilesError> {
    let images = run.images(release);
    let stores = BuiltImageStores::of(
        images
            .iter()
            .filter_map(|&(partition, image)| Some((partition, image.built()?))),
    );
    let super_layouts = run.super_layouts(release, published)?;
    let needed = storage_needed(&images, &super_layouts, &stores, stored_package);
    let free = storage.free_storage().await?;
    if free < needed {
        return Err(ReleaseFilesError::StorageShort(StorageShortage {
            needed,
            free,
            package_counted: stored_package.is_some(),
        }));
    }
    let mut stock_firmware = match &release.parts().stock_firmware {
        Some(firmware) => Some(open_stock_firmware(storage, firmware).await?),
        None => None,
    };
    let whole = whole_image_fetches(release, &images, super_layouts, published)?;
    let built = built_image_fetches(release, published, &stores)?;
    let picked = pick_release_files_the_page_cannot_download(
        storage,
        whole
            .iter()
            .chain(built.iter().flat_map(|image| &image.fetches)),
    )
    .await?;
    fetch_whole_images(storage, &whole, &picked).await?;
    storage
        .create_file(&StoredName::SharedStore.to_string(), stores.shared.len())
        .await?;
    for image in &built {
        let files = image.files;
        let own_name = StoredName::OwnStore(files.partition).to_string();
        storage.create_file(&own_name, files.unshared.len()).await?;
        fetch_image_files(storage, stock_firmware.as_mut(), image, &picked).await?;
        build_checked_image(storage, files, &stores.shared).await?;
        storage.remove_file(&own_name).await?;
        storage.report(&Progress::ImageChecked {
            partition: files.partition.as_str(),
        });
    }
    storage
        .remove_file(&StoredName::SharedStore.to_string())
        .await?;
    Ok(())
}

pub(crate) struct Fetch<'a> {
    pub(crate) file: RemoteFile,
    pub(crate) to: Destination<'a>,
}

fn storage_needed(
    images: &[(&PartitionName, &Image)],
    super_layouts: &[Fetch<'_>],
    stores: &BuiltImageStores<'_>,
    stored_package: Option<&Package>,
) -> u64 {
    let image_bytes: u64 = images.iter().map(|(_, image)| image.size()).sum();
    let layout_bytes: u64 = super_layouts.iter().map(|layout| layout.to.size).sum();
    let largest_own_store = stores
        .images
        .iter()
        .map(|files| files.unshared.len())
        .max()
        .unwrap_or(0);
    let store_bytes = stores.shared.len() + largest_own_store;
    let package_bytes = stored_package.map_or(0, |package| package.size);
    image_bytes + layout_bytes + store_bytes.max(package_bytes)
}

fn whole_image_fetches<'a>(
    release: &Release,
    images: &[(&'a PartitionName, &Image)],
    super_layouts: Vec<Fetch<'a>>,
    published: &PublishedRelease,
) -> Result<Vec<Fetch<'a>>, ReleaseFilesError> {
    let whole = images.iter().filter_map(|&(partition, image)| {
        let file = match image {
            Image::Shipped { .. } => published.asset(ReleaseFile::Image(partition)),
            Image::Fetched { blob } => Ok(RemoteFile::Source {
                source: release.parts().sources[blob.source.0].clone(),
                path: blob.path.clone(),
            }),
            Image::Built(_) => return None,
        };
        Some(file.map(|file| Fetch {
            file,
            to: Destination {
                name: StoredName::Image(partition),
                offset: 0,
                size: image.size(),
                sha256: image.sha256(),
            },
        }))
    });
    whole.chain(super_layouts.into_iter().map(Ok)).collect()
}

async fn fetch_whole_images(
    storage: &Storage,
    fetches: &[Fetch<'_>],
    picked: &PickedReleaseFiles,
) -> Result<(), ReleaseFilesError> {
    for fetch in fetches {
        storage
            .create_file(&fetch.to.name.to_string(), fetch.to.size)
            .await?;
    }
    let total = fetches.iter().map(|fetch| fetch.to.size).sum();
    let page = storage.clone();
    let arrived = ArrivedBytes::start(0, total, move |done| {
        page.report(&Progress::FetchingImages { done, total });
    });
    fetch_all(storage, fetches, picked, arrived).await?;
    storage.report(&Progress::FetchedImagesChecked);
    Ok(())
}

async fn fetch_all(
    storage: &Storage,
    fetches: &[Fetch<'_>],
    picked: &PickedReleaseFiles,
    arrived: ArrivedBytes,
) -> Result<(), ReleaseFilesError> {
    let mut started = Vec::new();
    for fetch in fetches {
        let arrival = arrived.file(fetch.to.size);
        if let Some(file) = picked.file(&fetch.file) {
            file.copy_to(storage, &fetch.to, &arrival)?;
        } else {
            let started_fetch = start_fetch(storage, &fetch.file, &fetch.to, arrival.clone())?;
            started.push((fetch, arrival, started_fetch));
        }
    }
    for (fetch, arrival, started_fetch) in started {
        let mut file = fetch.file.clone();
        let mut found = received(&file, started_fetch).await?;
        if let Some(object) = file.lfs_object()
            && found == lfs_pointer_facts(&fetch.to)
        {
            found = received(&object, start_fetch(storage, &object, &fetch.to, arrival)?).await?;
            file = object;
        }
        let (found_size, found) = found;
        if found_size != fetch.to.size || found != fetch.to.sha256 {
            return Err(ReleaseFilesError::FileDiffers(Box::new(FileDifference {
                file,
                listed: fetch.to.sha256,
                listed_size: fetch.to.size,
                found,
                found_size,
            })));
        }
    }
    Ok(())
}

fn start_fetch(
    storage: &Storage,
    file: &RemoteFile,
    to: &Destination<'_>,
    arrival: FileArrival,
) -> Result<(FileDigest, Download), HostFailure> {
    let digest = FileDigest::new(arrival);
    let download = storage.start_download(
        &file.address(),
        &to.name.to_string(),
        to.offset,
        to.size,
        &digest,
    )?;
    Ok((digest, download))
}

async fn received(
    file: &RemoteFile,
    (digest, download): (FileDigest, Download),
) -> Result<(u64, Sha256Digest), ReleaseFilesError> {
    match download.end().await? {
        Transfer::Received { value } => Ok((value, digest.digest())),
        Transfer::Failed { failure } => Err(ReleaseFilesError::Fetch {
            file: file.clone(),
            failure,
        }),
    }
}

fn lfs_pointer_facts(to: &Destination<'_>) -> (u64, Sha256Digest) {
    let pointer = lfs_pointer(to.sha256, to.size);
    (pointer.len() as u64, Sha256Digest::of(pointer.as_bytes()))
}

#[derive(Clone, Copy)]
struct NamedStore<'s, 'a> {
    store: &'s FileStore<'a>,
    name: StoredName<'s>,
}

#[derive(Clone, Copy)]
struct ImageStores<'s, 'a> {
    own: NamedStore<'s, 'a>,
    shared: NamedStore<'s, 'a>,
}

struct ImageFetches<'s, 'a> {
    files: &'s BuiltImageFiles<'a>,
    stock_files: Vec<StockFile<'a>>,
    fetches: Vec<Fetch<'a>>,
}

fn built_image_fetches<'s, 'a>(
    release: &Release,
    published: &PublishedRelease,
    stores: &'s BuiltImageStores<'a>,
) -> Result<Vec<ImageFetches<'s, 'a>>, ReleaseFilesError> {
    let mut fetched_shared = BTreeSet::new();
    stores
        .images
        .iter()
        .map(|files| {
            let own_name = StoredName::OwnStore(files.partition);
            let unshared = files.unshared.files().map(|file| (file, own_name));
            let shared_first = files
                .shared_used
                .iter()
                .filter(|file| fetched_shared.insert(file.sha256))
                .map(|file| (file, StoredName::SharedStore));
            let (stock_files, fetches) =
                split_store_files(release, published, unshared.chain(shared_first))?;
            Ok(ImageFetches {
                files,
                stock_files,
                fetches,
            })
        })
        .collect()
}

async fn fetch_image_files(
    storage: &Storage,
    stock_firmware: Option<&mut OpenedFirmware>,
    image: &ImageFetches<'_, '_>,
    picked: &PickedReleaseFiles,
) -> Result<(), ReleaseFilesError> {
    let partition = image.files.partition;
    if !image.stock_files.is_empty() {
        let firmware = stock_firmware.ok_or_else(|| ReleaseFilesError::StockFirmwareMissing {
            partition: partition.clone(),
        })?;
        firmware
            .read_files(storage, partition, &image.stock_files)
            .await?;
    }
    let total = image.fetches.iter().map(|fetch| fetch.to.size).sum();
    let page = storage.clone();
    let partition = partition.as_str().to_owned();
    let arrived = ArrivedBytes::start(0, total, move |done| {
        page.report(&Progress::FetchingFiles {
            partition: &partition,
            done,
            total,
        });
    });
    fetch_all(storage, &image.fetches, picked, arrived).await
}

async fn build_checked_image(
    storage: &Storage,
    files: &BuiltImageFiles<'_>,
    shared: &FileStore<'_>,
) -> Result<(), ReleaseFilesError> {
    let BuiltImageFiles {
        partition, image, ..
    } = *files;
    let image_name = StoredName::Image(partition).to_string();
    storage.create_file(&image_name, image.size).await?;
    let written = write_image(
        storage,
        partition,
        image,
        ImageStores {
            own: NamedStore {
                store: &files.unshared,
                name: StoredName::OwnStore(partition),
            },
            shared: NamedStore {
                store: shared,
                name: StoredName::SharedStore,
            },
        },
        &image_name,
    )?;
    if written.sha256 != image.sha256 {
        return Err(ReleaseFilesError::ImageDiffers {
            partition: partition.clone(),
            listed: image.sha256,
            found: written.sha256,
        });
    }
    Ok(())
}

fn split_store_files<'s, 'a: 's>(
    release: &Release,
    published: &PublishedRelease,
    files: impl IntoIterator<Item = (&'s StoredFile<'a>, StoredName<'a>)>,
) -> Result<(Vec<StockFile<'a>>, Vec<Fetch<'a>>), ReleaseFilesError> {
    let mut stock_files = Vec::new();
    let mut fetches = Vec::new();
    for (stored, name) in files {
        let to = Destination {
            name,
            offset: stored.offset,
            size: stored.size,
            sha256: stored.sha256,
        };
        match stored.origin {
            Origin::Stock { partition, path } => stock_files.push(StockFile {
                partition,
                path,
                to,
            }),
            Origin::Source { source, path } => fetches.push(Fetch {
                file: RemoteFile::Source {
                    source: release.parts().sources[source.0].clone(),
                    path: path.clone(),
                },
                to,
            }),
            Origin::FreeBytes { partition } => fetches.push(Fetch {
                file: published.asset(ReleaseFile::FreeBytes(partition))?,
                to,
            }),
            Origin::Delta { sha256 } => fetches.push(Fetch {
                file: published.asset(ReleaseFile::Delta(sha256))?,
                to,
            }),
        }
    }
    Ok((stock_files, fetches))
}

fn write_image(
    storage: &Storage,
    partition: &PartitionName,
    image: &BuiltImage,
    stores: ImageStores<'_, '_>,
    image_name: &str,
) -> Result<WrittenImage, ReleaseFilesError> {
    let own = open_store(storage, stores.own)?;
    let shared = open_store(storage, stores.shared)?;
    let stored_file = |sha256: &Sha256Digest| {
        own.file(sha256)
            .or_else(|| shared.file(sha256))
            .unwrap_or_else(|| {
                Err(ReleaseFilesError::FileNotInStore {
                    partition: partition.clone(),
                    sha256: *sha256,
                })
            })
    };
    let build_failure = |source| ReleaseFilesError::Build {
        partition: partition.clone(),
        source,
    };
    let files = image
        .unfree_files
        .iter()
        .map(|(path, content)| {
            FileToFill::new(
                path,
                content,
                stored_file(&content.fetched_file().sha256)?,
                |sha256| {
                    stored_file(sha256).map(|file| Rc::new(SourceBody::new(file)) as Rc<dyn Body>)
                },
            )
        })
        .collect::<Result<Vec<_>, ReleaseFilesError>>()?;
    let mut out = HostSink::open(storage.file(image_name)?)?;
    let mut shown = ShownPercent::default();
    let written = build_image(
        stored_file(&image.free_bytes.sha256)?,
        &files,
        &mut out,
        &mut |done| {
            if shown.advances_to(done, image.size) {
                storage.report(&Progress::Building {
                    partition: partition.as_str(),
                    done,
                    total: image.size,
                });
            }
        },
    )
    .map_err(build_failure)?;
    out.flush()
        .map_err(|error| build_failure(ImageBuildError::Output(error)))?;
    Ok(written)
}

struct OpenedStore<'s, 'a> {
    store: &'s FileStore<'a>,
    name: String,
    source: Rc<dyn ByteSource>,
}

impl OpenedStore<'_, '_> {
    fn file(&self, sha256: &Sha256Digest) -> Option<Result<Rc<dyn ByteSource>, ReleaseFilesError>> {
        let stored = self.store.file(sha256)?;
        let slice = SliceSource::new(Rc::clone(&self.source), stored.offset, stored.size);
        Some(match slice {
            Ok(slice) => Ok(Rc::new(slice)),
            Err(source) => Err(ReleaseFilesError::Stored {
                name: self.name.clone(),
                source,
            }),
        })
    }
}

fn open_store<'s, 'a>(
    storage: &Storage,
    NamedStore { store, name }: NamedStore<'s, 'a>,
) -> Result<OpenedStore<'s, 'a>, ReleaseFilesError> {
    let name = name.to_string();
    let source = match HostSource::open(storage.file(&name)?) {
        Ok(source) => source,
        Err(source) => return Err(ReleaseFilesError::Stored { name, source }),
    };
    Ok(OpenedStore {
        store,
        name,
        source: Rc::new(source),
    })
}
