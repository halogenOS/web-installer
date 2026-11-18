// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::io::Write;
use std::num::NonZeroUsize;
use std::rc::Rc;

use byte_io::{Body, ByteSource, CopyError, Scratch, SourceError, copy_body};
use fs_read::ImageFiles;
use sha256::Sha256Digest;

use crate::{CopyFileError, PackageError};

pub struct PartitionFiles {
    partition: String,
    files: ImageFiles,
    scratch: Scratch,
}

impl PartitionFiles {
    pub fn open(
        partition: &str,
        image: Rc<dyn ByteSource>,
        piece_len: NonZeroUsize,
    ) -> Result<Self, PackageError> {
        let files = ImageFiles::open(image).map_err(|source| PackageError::Filesystem {
            partition: partition.to_owned(),
            source,
        })?;
        Ok(Self {
            partition: partition.to_owned(),
            files,
            scratch: Scratch::new(piece_len),
        })
    }

    pub(crate) fn file(
        &mut self,
        path: &str,
        sha256: Sha256Digest,
    ) -> Result<Rc<dyn Body>, PackageError> {
        let (body, found) = self.file_with_digest(path)?;
        if found != sha256 {
            return Err(PackageError::FileMismatch {
                partition: self.partition.clone(),
                path: path.to_owned(),
                expected: sha256,
                found,
            });
        }
        Ok(body)
    }

    pub(crate) fn file_with_digest(
        &mut self,
        path: &str,
    ) -> Result<(Rc<dyn Body>, Sha256Digest), PackageError> {
        let body = self
            .files
            .file(path)
            .map_err(|source| PackageError::Filesystem {
                partition: self.partition.clone(),
                source,
            })?
            .ok_or_else(|| PackageError::FileMissing {
                partition: self.partition.clone(),
                path: path.to_owned(),
            })?;
        let digest = Sha256Digest::of_body(&*body, &mut self.scratch)
            .map_err(|source| self.read_failure(path, source))?;
        Ok((body, digest))
    }

    pub fn copy_file(
        &mut self,
        path: &str,
        sha256: Sha256Digest,
        sink: &mut dyn Write,
    ) -> Result<u64, CopyFileError> {
        let body = self.file(path, sha256)?;
        copy_body(&*body, sink, &mut self.scratch).map_err(|error| match error {
            CopyError::Source(source) => self.read_failure(path, source).into(),
            CopyError::Sink(source) => CopyFileError::Sink(source),
        })
    }

    #[must_use]
    pub(crate) fn read_failure(&self, path: &str, source: SourceError) -> PackageError {
        PackageError::FileRead {
            partition: self.partition.clone(),
            path: path.to_owned(),
            source,
        }
    }
}
