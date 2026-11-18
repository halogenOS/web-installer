// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::str::FromStr;

use release::{Release, ReleaseFile, ReleaseName};
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::github_releases::{GithubRelease, fetch_github_releases};
use crate::page::{ReleaseSite, Transfer, fetch_release_manifest};
use crate::page_failure::page_failure;
use crate::release_fetches::{ReleaseFetch, release_fetches};
use crate::release_files_failure::ReleaseFilesError;
use crate::remote_file::RemoteFile;

const SERIES_MARKER: &str = "XOS";

const DEVICE_MARKER: &str = "for";

#[derive(Serialize)]
struct ListedRelease {
    tag: String,
    device: ReleaseName,
    build: ReleaseName,
    series: Option<String>,
    fetches: Vec<ReleaseFetch>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReleasesToChoose {
    releases: Vec<ListedRelease>,
    unlisted_tag: Option<String>,
}

pub(crate) struct PublishedRelease {
    id: u64,
    tag: String,
    name: String,
    device: ReleaseName,
    published_at: String,
    assets: BTreeMap<String, String>,
}

impl PublishedRelease {
    fn of(release: GithubRelease) -> Option<Self> {
        let name = release.name?;
        let device = device_of(&name)?;
        let assets: BTreeMap<String, String> = release
            .assets
            .into_iter()
            .map(|asset| (asset.name, asset.url))
            .collect();
        assets
            .contains_key(&ReleaseFile::Manifest.name())
            .then_some(Self {
                id: release.id,
                tag: release.tag,
                name,
                device,
                published_at: release.published_at,
                assets,
            })
    }

    pub(crate) fn downloaded_asset_addresses(&self) -> impl Iterator<Item = &str> {
        let manifest = ReleaseFile::Manifest.name();
        self.assets
            .iter()
            .filter(move |(name, _)| **name != manifest)
            .map(|(_, address)| address.as_str())
    }

    pub(crate) fn asset(&self, file: ReleaseFile<'_>) -> Result<RemoteFile, ReleaseFilesError> {
        let name = file.name();
        match self.assets.get(&name) {
            Some(address) => Ok(RemoteFile::Asset {
                release: self.name.clone(),
                name,
                address: address.clone(),
            }),
            None => Err(ReleaseFilesError::AssetMissing {
                release: self.name.clone(),
                name,
            }),
        }
    }
}

#[wasm_bindgen(js_name = listReleases)]
pub async fn list_releases(
    site: ReleaseSite,
    linked_tag: Option<String>,
) -> Result<JsValue, JsValue> {
    let listed = list_releases_to_choose(&site, linked_tag.as_deref())
        .await
        .map_err(|failure| page_failure(&failure, failure.next_step()))?;
    Ok(serde_wasm_bindgen::to_value(&listed)?)
}

async fn list_releases_to_choose(
    site: &ReleaseSite,
    linked_tag: Option<&str>,
) -> Result<ReleasesToChoose, ReleaseFilesError> {
    let shown = releases_to_choose(fetch_github_releases(site).await?, linked_tag);
    let unlisted_tag = linked_tag
        .filter(|tag| shown.iter().all(|published| published.tag != *tag))
        .map(str::to_owned);
    let mut releases = Vec::new();
    for published in shown {
        let release = read_release(site, &published).await?;
        releases.push(ListedRelease {
            series: series_of(&published.name),
            build: release.parts().build.clone(),
            fetches: release_fetches(&published, &release),
            device: published.device,
            tag: published.tag,
        });
    }
    Ok(ReleasesToChoose {
        releases,
        unlisted_tag,
    })
}

pub(crate) async fn read_release_of_tag(
    site: &ReleaseSite,
    tag: &str,
) -> Result<(PublishedRelease, Release), ReleaseFilesError> {
    let published = installable(fetch_github_releases(site).await?)
        .into_iter()
        .find(|published| published.tag == tag)
        .ok_or(ReleaseFilesError::ReleaseMissing)?;
    let release = read_release(site, &published).await?;
    Ok((published, release))
}

async fn read_release(
    site: &ReleaseSite,
    published: &PublishedRelease,
) -> Result<Release, ReleaseFilesError> {
    let file = published.asset(ReleaseFile::Manifest)?;
    match fetch_release_manifest(site, published.id).await? {
        Transfer::Received { value } => {
            Release::from_json(&value).map_err(|source| ReleaseFilesError::Release {
                release: published.name.clone(),
                source,
            })
        }
        Transfer::Failed { failure } => Err(ReleaseFilesError::Fetch { file, failure }),
    }
}

fn installable(releases: Vec<GithubRelease>) -> Vec<PublishedRelease> {
    releases
        .into_iter()
        .filter_map(PublishedRelease::of)
        .collect()
}

fn releases_to_choose(
    releases: Vec<GithubRelease>,
    linked_tag: Option<&str>,
) -> Vec<PublishedRelease> {
    let mut published = installable(releases);
    match linked_tag.and_then(|tag| published.iter().position(|release| release.tag == tag)) {
        Some(linked) => vec![published.swap_remove(linked)],
        None => newest_of_every_device(published),
    }
}

fn newest_of_every_device(mut published: Vec<PublishedRelease>) -> Vec<PublishedRelease> {
    published.sort_by(|left, right| {
        left.device
            .cmp(&right.device)
            .then_with(|| right.published_at.cmp(&left.published_at))
            .then_with(|| right.name.cmp(&left.name))
    });
    published.dedup_by(|later, newest| later.device == newest.device);
    published
}

fn device_of(name: &str) -> Option<ReleaseName> {
    let (start, codename) = name.trim_end().rsplit_once(' ')?;
    let is_codename = codename
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if !start.ends_with(DEVICE_MARKER) || !is_codename {
        return None;
    }
    ReleaseName::from_str(codename).ok()
}

fn series_of(name: &str) -> Option<String> {
    name.match_indices(SERIES_MARKER)
        .find_map(|(start, marker)| {
            let rest = name[start + marker.len()..]
                .strip_prefix(|character: char| character == '-' || character.is_whitespace())?;
            leading_version(rest.strip_prefix('v').unwrap_or(rest)).map(str::to_owned)
        })
}

fn leading_version(text: &str) -> Option<&str> {
    let end = text
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(text.len());
    let version = text[..end].split("..").next()?.trim_end_matches('.');
    version
        .starts_with(|character: char| character.is_ascii_digit())
        .then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github_releases::GithubAsset;

    fn release(name: &str, published_at: &str) -> GithubRelease {
        GithubRelease {
            id: 1,
            tag: name.to_owned(),
            name: Some(name.to_owned()),
            published_at: published_at.to_owned(),
            assets: vec![GithubAsset {
                name: "manifest.json".to_owned(),
                url: format!("https://example.org/{published_at}/manifest.json"),
            }],
        }
    }

    #[test]
    fn the_device_is_the_codename_after_for_at_the_end_of_the_name() {
        for (name, device) in [
            (
                "[release build] XOS-17.0 16/11/2026 for Spacewar",
                Some("Spacewar"),
            ),
            ("[test build] XOS-16.2 28/07/2026 for Pong  ", Some("Pong")),
            ("XOS-17.0 for sdk_phone64", Some("sdk_phone64")),
            ("XOS-17.0 for Spacewar 16/11/2026", None),
            ("XOS-17.0 for Spacewar-beta", None),
            ("XOS-17.0 GSI", None),
        ] {
            assert_eq!(
                device_of(name).as_ref().map(ReleaseName::as_str),
                device,
                "{name}"
            );
        }
    }

    #[test]
    fn the_series_is_the_version_after_xos_in_the_name() {
        for (name, series) in [
            (
                "[release build] XOS-17.0 16/11/2026 for Spacewar",
                Some("17.0"),
            ),
            ("XOS v12.1 for guacamole", Some("12.1")),
            ("XOS 16.2.1. for Pong", Some("16.2.1")),
            ("XOS-17..1 for Pong", Some("17")),
            ("XOS-.5 for Pong", None),
            ("halogenOS_Spacewar-17.0 for Spacewar", None),
        ] {
            assert_eq!(series_of(name).as_deref(), series, "{name}");
        }
    }

    #[test]
    fn the_newest_installable_release_of_each_device_is_listed_in_device_order() {
        let mut without_manifest = release("XOS-17.1 for Spacewar", "2026-11-17T08:00:00Z");
        without_manifest.assets.clear();
        let newest = newest_of_every_device(installable(vec![
            release("XOS-17.0 for Spacewar", "2026-11-16T09:52:00Z"),
            release("XOS-16.2 for Pong", "2026-07-28T06:50:51Z"),
            without_manifest,
            release("XOS-17.0 for Pong", "2026-07-28T06:50:51Z"),
            release("XOS-16.2 for Spacewar", "2026-07-07T20:43:15Z"),
            release("XOS-17.0 GSI", "2026-11-17T09:00:00Z"),
        ]));
        let listed: Vec<(&str, &str)> = newest
            .iter()
            .map(|published| (published.device.as_str(), published.name.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                ("Pong", "XOS-17.0 for Pong"),
                ("Spacewar", "XOS-17.0 for Spacewar"),
            ]
        );
    }

    #[test]
    fn a_linked_tag_lists_its_release_alone_and_an_unknown_one_the_newest_of_every_device() {
        let releases = || {
            vec![
                release("XOS-17.0 for Spacewar", "2026-11-16T09:52:00Z"),
                release("XOS-16.2 for Spacewar", "2026-07-07T20:43:15Z"),
                release("XOS-17.0 for Pong", "2026-07-28T06:50:51Z"),
            ]
        };
        let newest = ["XOS-17.0 for Pong", "XOS-17.0 for Spacewar"];
        for (linked_tag, expected) in [
            (
                Some("XOS-16.2 for Spacewar"),
                &["XOS-16.2 for Spacewar"][..],
            ),
            (Some("XOS-15.0 for Spacewar"), &newest[..]),
            (None, &newest[..]),
        ] {
            let listed: Vec<String> = releases_to_choose(releases(), linked_tag)
                .into_iter()
                .map(|published| published.name)
                .collect();
            assert_eq!(listed, expected, "{linked_tag:?}");
        }
    }

    #[test]
    fn a_file_of_the_release_is_the_asset_of_its_name() {
        let published = PublishedRelease::of(release("XOS-17.0 for Pong", "2026-07-28T06:50:51Z"))
            .expect("the release is installable");
        let manifest = published.asset(ReleaseFile::Manifest).unwrap();
        assert_eq!(
            manifest.address(),
            "https://example.org/2026-07-28T06:50:51Z/manifest.json"
        );
        assert!(matches!(
            published.asset(ReleaseFile::SuperEmpty),
            Err(ReleaseFilesError::AssetMissing { name, .. }) if name == "super_empty.img"
        ));
    }

    #[test]
    fn the_page_downloads_every_asset_but_the_manifest_from_github() {
        let mut listed = release("XOS-17.0 for Pong", "2026-07-28T06:50:51Z");
        listed.assets.push(GithubAsset {
            name: "system.img".to_owned(),
            url: "https://example.org/system.img".to_owned(),
        });
        let published = PublishedRelease::of(listed).expect("the release is installable");
        assert_eq!(
            published.downloaded_asset_addresses().collect::<Vec<_>>(),
            ["https://example.org/system.img"]
        );
    }
}
