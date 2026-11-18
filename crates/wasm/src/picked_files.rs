// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::rc::Rc;

use byte_io::{ByteSource, READ_PIECE_LEN, Scratch, SourceBody, SourceError, visit_pieces};
use release::StockPackage;
use serde::Serialize;
use sha256::{RunningDigest, Sha256Digest};

use crate::host_source::HostSource;
use crate::page::{PickedFile, Storage};
use crate::page_failure::PageFailure;
use crate::progress::{AwaitedFile, Progress, ShownPercent};
use crate::release_files_failure::{ReleaseFilesError, wrong_digest_failure, wrong_length_failure};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AskedFiles {
    StockFirmware,
    ReleaseFiles,
}

#[derive(Debug, Serialize)]
pub(crate) struct AskedFile<'a> {
    pub(crate) file_name: &'a str,
    pub(crate) address: &'a str,
    pub(crate) sha256: Sha256Digest,
    pub(crate) size: u64,
}

impl<'a> From<&'a StockPackage> for AskedFile<'a> {
    fn from(package: &'a StockPackage) -> Self {
        Self {
            file_name: package.file_name.as_str(),
            address: package.address.as_str(),
            sha256: package.sha256,
            size: package.size,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PickedFileCheck {
    Length,
    Sha256,
}

pub(crate) async fn pick_files(
    storage: &Storage,
    asked: AskedFiles,
    awaited: &[AskedFile<'_>],
    check: PickedFileCheck,
) -> Result<Vec<Rc<dyn ByteSource>>, ReleaseFilesError> {
    let mut picked: Vec<Option<Rc<dyn ByteSource>>> = vec![None; awaited.len()];
    let mut wrong_picks = Vec::new();
    loop {
        storage.report(&Progress::ChoosingFiles {
            asked,
            files: awaited
                .iter()
                .zip(&picked)
                .map(|(file, source)| AwaitedFile {
                    file,
                    picked: source.is_some(),
                })
                .collect(),
            wrong_picks: std::mem::take(&mut wrong_picks),
        });
        let picks = storage
            .picked_files()
            .await?
            .into_iter()
            .map(|PickedFile { name, file }| match HostSource::open(file) {
                Ok(source) => Ok((name, Rc::new(source) as Rc<dyn ByteSource>)),
                Err(source) => Err(ReleaseFilesError::PickedRead { name, source }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut digests = PickedFileDigests {
            storage,
            done: 0,
            total: picks
                .iter()
                .map(|(_, source)| source.len())
                .filter(|&len| has_awaited_length(awaited, len))
                .sum(),
            shown: ShownPercent::default(),
        };
        for (name, source) in picks {
            match recognize(awaited, &name, &source, check, &mut digests) {
                Ok(Ok(index)) if picked[index].is_none() => picked[index] = Some(source),
                Ok(Ok(_picked_before)) => {}
                Ok(Err(refusal)) => wrong_picks.push(refusal),
                Err(source) => return Err(ReleaseFilesError::PickedRead { name, source }),
            }
        }
        if let Some(sources) = picked.iter().cloned().collect() {
            return Ok(sources);
        }
    }
}

fn recognize(
    awaited: &[AskedFile<'_>],
    name: &str,
    source: &Rc<dyn ByteSource>,
    check: PickedFileCheck,
    digests: &mut PickedFileDigests<'_>,
) -> Result<Result<usize, PageFailure>, SourceError> {
    let len = source.len();
    let has_len = |file: &AskedFile<'_>| file.size == len;
    let index = match check {
        PickedFileCheck::Length => awaited
            .iter()
            .position(has_len)
            .ok_or_else(|| wrong_length_failure(name, len)),
        PickedFileCheck::Sha256 if !has_awaited_length(awaited, len) => {
            Err(wrong_length_failure(name, len))
        }
        PickedFileCheck::Sha256 => {
            let found = digests.read(source)?;
            awaited
                .iter()
                .position(|file| has_len(file) && file.sha256 == found)
                .ok_or_else(|| wrong_digest_failure(name, found))
        }
    };
    Ok(index)
}

fn has_awaited_length(awaited: &[AskedFile<'_>], len: u64) -> bool {
    awaited.iter().any(|file| file.size == len)
}

struct PickedFileDigests<'s> {
    storage: &'s Storage,
    done: u64,
    total: u64,
    shown: ShownPercent,
}

impl PickedFileDigests<'_> {
    fn read(&mut self, source: &Rc<dyn ByteSource>) -> Result<Sha256Digest, SourceError> {
        let mut digest = RunningDigest::default();
        let _read: u64 = visit_pieces(
            &SourceBody::new(Rc::clone(source)),
            &mut Scratch::new(READ_PIECE_LEN),
            |piece| {
                digest.update(piece);
                self.done += piece.len() as u64;
                if self.shown.advances_to(self.done, self.total) {
                    self.storage.report(&Progress::CheckingPickedFiles {
                        done: self.done,
                        total: self.total,
                    });
                }
                Ok::<(), SourceError>(())
            },
        )?;
        Ok(digest.finish())
    }
}
