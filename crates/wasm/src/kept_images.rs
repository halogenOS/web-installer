// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeSet;
use std::rc::Rc;

use byte_io::{READ_PIECE_LEN, Scratch, SourceBody};
use sha256::Sha256Digest;

use crate::host_source::HostSource;
use crate::page::Storage;
use crate::release_files_failure::ReleaseFilesError;
use crate::release_images::{Destination, StoredName};

/// The images that a run before stored and this run asks for again.
pub(crate) struct KeptImages {
    checked: BTreeSet<String>,
    /// Every kept image already takes its room, also one whose bytes the run writes again in place.
    pub(crate) bytes: u64,
}

impl KeptImages {
    /// Whether the stored image has the size and the SHA-256 that the release lists for it.
    pub(crate) fn has(&self, name: StoredName<'_>) -> bool {
        self.checked.contains(&name.to_string())
    }
}

/// Removes every stored file of a run before except the images this run asks for, and checks the
/// bytes of each kept image, since a run may store bytes before it finds that they differ.
pub(crate) async fn keep_stored_images(
    storage: &Storage,
    images: &[Destination<'_>],
) -> Result<KeptImages, ReleaseFilesError> {
    let kept = storage
        .keep_files(images.iter().map(|image| image.name.to_string()).collect())
        .await?;
    let mut checked = BTreeSet::new();
    for image in images {
        let name = image.name.to_string();
        let same_size = kept
            .iter()
            .any(|file| file.name == name && file.size == image.size);
        if same_size && stored_digest(storage, &name)? == image.sha256 {
            checked.extend([name]);
        }
    }
    Ok(KeptImages {
        checked,
        bytes: kept.iter().map(|file| file.size).sum(),
    })
}

fn stored_digest(storage: &Storage, name: &str) -> Result<Sha256Digest, ReleaseFilesError> {
    let unreadable = |source| ReleaseFilesError::Stored {
        name: name.to_owned(),
        source,
    };
    let stored = HostSource::open(storage.file(name)?).map_err(unreadable)?;
    Sha256Digest::of_body(
        &SourceBody::new(Rc::new(stored)),
        &mut Scratch::new(READ_PIECE_LEN),
    )
    .map_err(unreadable)
}
