// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use byte_io::{READ_PIECE_LEN, SourceError, clamp_len, to_len};
use image_archive::{ArchiveError, ForwardWalk, ImageRanges, PieceLen};
use release::PartitionName;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::arrived_bytes::ArrivedBytes;
use crate::image_archive_files::{PartDownload, StockArchive, StockPart, write_image};
use crate::page::{Storage, Transfer};
use crate::progress::{BlockDownload, BlockEdge, Progress};
use crate::range_arrival::{RangeArrival, RangeEnd};
use crate::release_files_failure::ReleaseFilesError;
use crate::remote_file::RemoteFile;

const MILLISECONDS_PER_SECOND: f64 = 1000.0;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn monotonic_milliseconds() -> f64;
}

#[derive(Debug)]
pub(crate) struct RangeEndedEarly {
    file: RemoteFile,
    range: Range<u64>,
    received: u64,
}

impl fmt::Display for RangeEndedEarly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            file,
            range,
            received,
        } = self;
        write!(
            f,
            "The server sent {received} of the {} bytes from byte {} of {file}",
            range.end - range.start,
            range.start
        )
    }
}

pub(crate) async fn extract_from_ranges(
    storage: &Storage,
    archive: &StockArchive,
    partition: &PartitionName,
    name: &str,
    built: &PartitionName,
) -> Result<(), ReleaseFilesError> {
    let ranges = archive
        .archive
        .image_ranges(partition.as_str())
        .map_err(archive_failure)?;
    match ranges {
        ImageRanges::FromBlockStart(walk) => {
            let range = walk.range();
            let download = block_download(&range, BlockEdge::Start);
            let arrived = report_arrival(storage, download);
            let walk = Rc::new(RefCell::new(walk));
            fetch_range(storage, &archive.parts, range, Some(&walk), &arrived).await?;
            report_checked(storage, download, &arrived);
            write_image(storage, name, built, |sink, progress| {
                archive
                    .archive
                    .extract_image(archive.readers(), partition.as_str(), sink, progress)
            })
        }
        ImageRanges::ToBlockEnd(mut search) => {
            let download = block_download(&search.range(), BlockEdge::End);
            let arrived = report_arrival(storage, download);
            let mut piece_len = PieceLen::default();
            loop {
                let range = search
                    .next_piece(piece_len.get())
                    .map_err(archive_failure)?;
                let started = monotonic_milliseconds();
                fetch_range(storage, &archive.parts, range.clone(), None, &arrived).await?;
                piece_len.follow_speed(
                    downloaded_bytes(&archive.parts, &range),
                    elapsed_since(started),
                );
                for start in search.read_piece(read_range(&archive.parts, &range)?) {
                    let written = write_image(storage, name, built, |sink, progress| {
                        archive.archive.extract_image_from(
                            archive.readers(),
                            partition.as_str(),
                            start,
                            sink,
                            progress,
                        )
                    });
                    match written {
                        Err(ReleaseFilesError::ImageArchive(error))
                            if matches!(
                                *error,
                                ArchiveError::ImageDiffers { .. } | ArchiveError::Data(_)
                            ) => {}
                        written => {
                            written?;
                            report_checked(storage, download, &arrived);
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

fn block_download(range: &Range<u64>, read_from: BlockEdge) -> BlockDownload {
    BlockDownload {
        downloaded: 0,
        block: range.end - range.start,
        read_from,
    }
}

fn report_arrival(storage: &Storage, download: BlockDownload) -> ArrivedBytes {
    let page = storage.clone();
    ArrivedBytes::start(download.downloaded, download.block, move |downloaded| {
        page.report(&Progress::FetchingStockFirmware(BlockDownload {
            downloaded,
            ..download
        }));
    })
}

fn report_checked(storage: &Storage, download: BlockDownload, arrived: &ArrivedBytes) {
    storage.report(&Progress::StockFirmwareBlockChecked(BlockDownload {
        downloaded: arrived.done(),
        ..download
    }));
}

async fn fetch_range(
    storage: &Storage,
    parts: &[StockPart],
    range: Range<u64>,
    walk: Option<&Rc<RefCell<ForwardWalk>>>,
    arrived: &ArrivedBytes,
) -> Result<(), ReleaseFilesError> {
    for (part, in_part) in ranges_in_parts(parts, &range) {
        match &part.download {
            Some(download) => download_range(storage, download, in_part, walk, arrived).await?,
            None => {
                if let Some(walk) = walk {
                    walk_picked_part(part, in_part, walk)?;
                }
            }
        }
        if walk.is_some_and(|walk| walk.borrow().is_complete()) {
            break;
        }
    }
    Ok(())
}

async fn download_range(
    storage: &Storage,
    download: &PartDownload,
    range: Range<u64>,
    walk: Option<&Rc<RefCell<ForwardWalk>>>,
    arrived: &ArrivedBytes,
) -> Result<(), ReleaseFilesError> {
    let len = range.end - range.start;
    let arrival = RangeArrival::new(len, arrived.file(len), walk);
    let file = RemoteFile::Stock(download.address.clone());
    let transfer = storage
        .start_range_download(
            download.address.as_str(),
            &download.stored,
            &range,
            &arrival,
        )?
        .end()
        .await?;
    match transfer {
        Transfer::Failed { failure } => Err(ReleaseFilesError::Fetch { file, failure }),
        Transfer::Received {
            value: _stored_as_taken,
        } => match arrival.end().map_err(archive_failure)? {
            RangeEnd::Complete => Ok(()),
            RangeEnd::EndedEarly { received } => {
                Err(ReleaseFilesError::RangeEndedEarly(RangeEndedEarly {
                    file,
                    range,
                    received,
                }))
            }
        },
    }
}

fn walk_picked_part(
    part: &StockPart,
    range: Range<u64>,
    walk: &Rc<RefCell<ForwardWalk>>,
) -> Result<(), ReleaseFilesError> {
    let mut at = range.start;
    while at < range.end && !walk.borrow().is_complete() {
        let mut piece = vec![0; clamp_len(range.end - at, READ_PIECE_LEN.get())];
        part.bytes
            .read_at(at, &mut piece)
            .map_err(part_unreadable)?;
        let _taken_up_to_the_end_of_the_walk: usize =
            walk.borrow_mut().take(&piece).map_err(archive_failure)?;
        at += to_len(piece.len());
    }
    Ok(())
}

fn read_range(parts: &[StockPart], range: &Range<u64>) -> Result<Vec<u8>, ReleaseFilesError> {
    let mut bytes = Vec::new();
    for (part, in_part) in ranges_in_parts(parts, range) {
        let mut piece = vec![0; clamp_len(in_part.end - in_part.start, usize::MAX)];
        part.bytes
            .read_at(in_part.start, &mut piece)
            .map_err(part_unreadable)?;
        bytes.extend(piece);
    }
    Ok(bytes)
}

fn downloaded_bytes(parts: &[StockPart], range: &Range<u64>) -> u64 {
    ranges_in_parts(parts, range)
        .filter(|(part, _)| part.download.is_some())
        .map(|(_, in_part)| in_part.end - in_part.start)
        .sum()
}

fn ranges_in_parts<'a>(
    parts: &'a [StockPart],
    range: &Range<u64>,
) -> impl Iterator<Item = (&'a StockPart, Range<u64>)> {
    parts
        .iter()
        .scan(0, |part_start, part| {
            let start = *part_start;
            *part_start += part.bytes.len();
            Some((part, start))
        })
        .filter_map(move |(part, part_start)| {
            let start = range.start.max(part_start);
            let end = range.end.min(part_start + part.bytes.len());
            (start < end).then(|| (part, start - part_start..end - part_start))
        })
}

fn elapsed_since(start: f64) -> Duration {
    let seconds = (monotonic_milliseconds() - start) / MILLISECONDS_PER_SECOND;
    Duration::try_from_secs_f64(seconds).unwrap_or_default()
}

fn archive_failure(error: ArchiveError) -> ReleaseFilesError {
    ReleaseFilesError::ImageArchive(Box::new(error))
}

fn part_unreadable(source: SourceError) -> ReleaseFilesError {
    archive_failure(ArchiveError::Read(source.into()))
}
