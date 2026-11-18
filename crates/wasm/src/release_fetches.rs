// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use release::{Release, RepositoryName, Source, StockPackageAddress, StockPackageName};
use serde::Serialize;

use crate::releases::PublishedRelease;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ReleaseFetch {
    BuildFile {
        address: String,
    },
    SourceFiles {
        repository: RepositoryName,
        address: String,
    },
    SourceLfsObjects {
        repository: RepositoryName,
        address: String,
    },
    StockFirmware {
        file_name: StockPackageName,
        address: StockPackageAddress,
    },
}

pub(crate) fn release_fetches(
    published: &PublishedRelease,
    release: &Release,
) -> Vec<ReleaseFetch> {
    let parts = release.parts();
    let build_files =
        published
            .downloaded_asset_addresses()
            .map(|address| ReleaseFetch::BuildFile {
                address: address.to_owned(),
            });
    let source_files = parts.sources.iter().flat_map(|source| match source {
        Source::Git(git) => vec![
            ReleaseFetch::SourceFiles {
                repository: git.repository.clone(),
                address: git.commit_address(),
            },
            ReleaseFetch::SourceLfsObjects {
                repository: git.repository.clone(),
                address: git.lfs_commit_address(),
            },
        ],
        Source::OtaPackage(_) | Source::ImageArchive { .. } => source
            .stock_packages()
            .iter()
            .map(|package| ReleaseFetch::StockFirmware {
                file_name: package.file_name.clone(),
                address: package.address.clone(),
            })
            .collect(),
    });
    build_files.chain(source_files).collect()
}
