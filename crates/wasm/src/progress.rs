// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use release::{GitRepository, Image, PartitionName, Release, Source, Step, StockPackage};
use serde::Serialize;

use crate::page_failure::PageFailure;
use crate::picked_files::{AskedFile, AskedFiles};
use crate::release_images::ImagesFor;

#[derive(Debug, Serialize)]
#[serde(
    tag = "stage",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Progress<'a> {
    Connecting,
    Checking(ReleasePlan<'a>),
    ReadingRelease,
    ReleaseRead(ReleasePlan<'a>),
    ChoosingFiles {
        asked: AskedFiles,
        files: Vec<AwaitedFile<'a>>,
        wrong_picks: Vec<PageFailure>,
    },
    CheckingPickedFiles {
        done: u64,
        total: u64,
    },
    FetchingStockFirmware(BlockDownload),
    StockFirmwareChecked,
    StockFirmwareBlockChecked(BlockDownload),
    UnpackingStockImage {
        partition: &'a str,
        done: u64,
        total: u64,
    },
    FetchingImages {
        done: u64,
        total: u64,
    },
    FetchedImagesChecked,
    ReadingStockFiles {
        partition: &'a str,
        done: u64,
        total: u64,
    },
    FetchingFiles {
        partition: &'a str,
        done: u64,
        total: u64,
    },
    Building {
        partition: &'a str,
        done: u64,
        total: u64,
    },
    ImageChecked {
        partition: &'a str,
    },
    Installing {
        index: usize,
        count: usize,
        step: &'a Step,
    },
    Sending {
        done: u64,
        total: u64,
    },
    Done,
    WritingPackage {
        done: u64,
        total: u64,
    },
    PackageSaved,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockDownload {
    pub(crate) downloaded: u64,
    pub(crate) block: u64,
    pub(crate) read_from: BlockEdge,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockEdge {
    Start,
    End,
}

#[derive(Debug, Serialize)]
pub(crate) struct AwaitedFile<'a> {
    pub(crate) file: &'a AskedFile<'a>,
    pub(crate) picked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReleasePlan<'a> {
    device: &'a str,
    build: &'a str,
    shipped: Vec<&'a str>,
    built: Vec<&'a str>,
    sources: Vec<&'a GitRepository>,
    stock_files: Vec<&'a StockPackage>,
    install: Vec<&'a Step>,
}

impl<'a> ReleasePlan<'a> {
    pub(crate) fn of(release: &'a Release, run: ImagesFor) -> Self {
        let parts = release.parts();
        let names = |images: Vec<(&'a PartitionName, &'a Image)>| {
            images
                .into_iter()
                .map(|(partition, _)| partition.as_str())
                .collect()
        };
        let (built, shipped) = run
            .images(release)
            .into_iter()
            .partition(|(_, image)| matches!(image, Image::Built(_)));
        Self {
            device: parts.device.as_str(),
            build: parts.build.as_str(),
            shipped: names(shipped),
            built: names(built),
            sources: parts.sources.iter().filter_map(Source::git).collect(),
            stock_files: parts
                .sources
                .iter()
                .flat_map(Source::stock_packages)
                .collect(),
            install: run.steps(release),
        }
    }
}

#[derive(Default)]
pub(crate) struct ShownPercent(Option<u64>);

impl ShownPercent {
    pub(crate) fn advances_to(&mut self, done: u64, total: u64) -> bool {
        let percent = Some(done * 100 / total.max(1));
        let advanced = self.0 != percent;
        self.0 = percent;
        advanced
    }
}
