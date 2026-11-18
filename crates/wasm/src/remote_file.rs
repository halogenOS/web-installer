// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;

use release::{RepositoryPath, Source, StockPackageAddress};

#[derive(Clone, Debug)]
pub(crate) enum RemoteFile {
    Source {
        source: Source,
        path: RepositoryPath,
    },
    Lfs {
        source: Source,
        path: RepositoryPath,
    },
    Asset {
        release: String,
        name: String,
        address: String,
    },
    Stock(StockPackageAddress),
}

impl RemoteFile {
    pub(crate) fn address(&self) -> String {
        match self {
            Self::Source { source, path } => source.address(path),
            Self::Lfs { source, path } => source.lfs_address(path),
            Self::Asset { address, .. } => address.clone(),
            Self::Stock(address) => address.as_str().to_owned(),
        }
    }

    pub(crate) fn lfs_object(&self) -> Option<Self> {
        match self {
            Self::Source { source, path } => Some(Self::Lfs {
                source: source.clone(),
                path: path.clone(),
            }),
            Self::Lfs { .. } | Self::Asset { .. } | Self::Stock(_) => None,
        }
    }
}

impl fmt::Display for RemoteFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { source, path } => {
                write!(f, "{path} at {} {}", source.repository, source.commit)
            }
            Self::Lfs { source, path } => write!(
                f,
                "the Git LFS object of {path} at {} {}",
                source.repository, source.commit
            ),
            Self::Asset { release, name, .. } => write!(f, "{name} of the release {release}"),
            Self::Stock(address) => address.fmt(f),
        }
    }
}
