// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::io::Write;
use std::rc::Rc;

use byte_io::{ByteSource, CopyError, READ_PIECE_LEN, Scratch, SourceBody, visit_pieces};

use crate::arrived_bytes::FileArrival;
use crate::host_sink::HostSink;
use crate::page::Storage;
use crate::picked_files::{AskedFile, AskedFiles, PickedFileCheck, pick_files};
use crate::release_files_failure::ReleaseFilesError;
use crate::release_images::{Destination, Fetch};
use crate::remote_file::RemoteFile;

#[derive(Default)]
pub(crate) struct PickedReleaseFiles(BTreeMap<String, PickedReleaseFile>);

pub(crate) struct PickedReleaseFile {
    name: String,
    source: Rc<dyn ByteSource>,
}

impl PickedReleaseFiles {
    pub(crate) fn file(&self, file: &RemoteFile) -> Option<&PickedReleaseFile> {
        let RemoteFile::Asset { address, .. } = file else {
            return None;
        };
        self.0.get(address)
    }
}

pub(crate) async fn pick_release_files_the_page_cannot_download<'f, 'a: 'f>(
    storage: &Storage,
    fetches: impl IntoIterator<Item = &'f Fetch<'a>>,
) -> Result<PickedReleaseFiles, ReleaseFilesError> {
    let assets: BTreeMap<&str, AskedFile<'_>> = fetches
        .into_iter()
        .filter_map(|fetch| {
            let RemoteFile::Asset { name, address, .. } = &fetch.file else {
                return None;
            };
            let asked = AskedFile {
                file_name: name,
                address,
                sha256: fetch.to.sha256,
                size: fetch.to.size,
            };
            Some((address.as_str(), asked))
        })
        .collect();
    let Some(&probed) = assets.keys().next() else {
        return Ok(PickedReleaseFiles::default());
    };
    if storage.can_download(probed).await? {
        return Ok(PickedReleaseFiles::default());
    }
    let asked: Vec<AskedFile<'_>> = assets.into_values().collect();
    let picked = pick_files(
        storage,
        AskedFiles::ReleaseFiles,
        &asked,
        PickedFileCheck::Content,
    )
    .await?;
    Ok(PickedReleaseFiles(
        asked
            .into_iter()
            .zip(picked)
            .map(|(file, source)| {
                let picked = PickedReleaseFile {
                    name: file.file_name.to_owned(),
                    source,
                };
                (file.address.to_owned(), picked)
            })
            .collect(),
    ))
}

impl PickedReleaseFile {
    pub(crate) fn copy_to(
        &self,
        storage: &Storage,
        to: &Destination<'_>,
        arrival: &FileArrival,
    ) -> Result<(), ReleaseFilesError> {
        let stored = to.name.to_string();
        let store_write = |source| ReleaseFilesError::StoreWrite {
            name: stored.clone(),
            source,
        };
        let mut sink = HostSink::open(storage.file_from(&stored, to.offset)?)?;
        let _copied: u64 = visit_pieces(
            &SourceBody::new(Rc::clone(&self.source)),
            &mut Scratch::new(READ_PIECE_LEN),
            |piece| {
                sink.write_all(piece).map_err(CopyError::Sink)?;
                arrival.add(piece.len() as u64);
                Ok(())
            },
        )
        .map_err(|error| match error {
            CopyError::Source(source) => ReleaseFilesError::PickedRead {
                name: self.name.clone(),
                source,
            },
            CopyError::Sink(source) => store_write(source),
        })?;
        sink.flush().map_err(store_write)
    }
}
