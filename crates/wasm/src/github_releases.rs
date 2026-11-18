// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use serde::Deserialize;

use crate::page::{ReleaseSite, Transfer, fetch_releases};
use crate::release_files_failure::ReleaseFilesError;

#[derive(Deserialize)]
struct ReleaseList {
    releases: Vec<GithubRelease>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubRelease {
    pub(crate) id: u64,
    pub(crate) tag: String,
    pub(crate) name: Option<String>,
    pub(crate) published_at: String,
    pub(crate) assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
pub(crate) struct GithubAsset {
    pub(crate) name: String,
    pub(crate) url: String,
}

pub(crate) async fn fetch_github_releases(
    site: &ReleaseSite,
) -> Result<Vec<GithubRelease>, ReleaseFilesError> {
    let text = match fetch_releases(site).await? {
        Transfer::Received { value } => value,
        Transfer::Failed { failure } => return Err(ReleaseFilesError::ReleasesFetch { failure }),
    };
    let list: ReleaseList = serde_json::from_str(&text)
        .map_err(|source| ReleaseFilesError::ReleasesUnreadable { source })?;
    Ok(list.releases)
}
