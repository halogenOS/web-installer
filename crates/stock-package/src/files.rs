// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::io::Write;
use std::num::NonZeroUsize;
use std::rc::Rc;

use byte_io::{Body, ByteSource, CopyError, Scratch, SourceError, read_all, visit_pieces};
use fs_read::ImageFiles;
use sha256::{RunningDigest, Sha256Digest};

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

    pub(crate) fn file_with_digest(
        &mut self,
        path: &str,
    ) -> Result<(Rc<dyn Body>, Sha256Digest), PackageError> {
        let body = self.find_file(path)?;
        let digest = Sha256Digest::of_body(&*body, &mut self.scratch)
            .map_err(|source| self.read_failure(path, source))?;
        Ok((body, digest))
    }

    pub(crate) fn file_bytes(
        &mut self,
        path: &str,
        sha256: Sha256Digest,
    ) -> Result<Vec<u8>, PackageError> {
        let body = self.find_file(path)?;
        let bytes = read_all(&*body).map_err(|source| self.read_failure(path, source))?;
        self.check_digest(path, sha256, Sha256Digest::of(&bytes))?;
        Ok(bytes)
    }

    pub fn copy_file(
        &mut self,
        path: &str,
        sha256: Sha256Digest,
        sink: &mut dyn Write,
    ) -> Result<u64, CopyFileError> {
        let body = self.find_file(path)?;
        let mut digest = RunningDigest::default();
        let len = visit_pieces(&*body, &mut self.scratch, |piece| {
            digest.update(piece);
            sink.write_all(piece).map_err(CopyError::Sink)
        })
        .map_err(|error| match error {
            CopyError::Source(source) => self.read_failure(path, source).into(),
            CopyError::Sink(source) => CopyFileError::Sink(source),
        })?;
        self.check_digest(path, sha256, digest.finish())?;
        Ok(len)
    }

    fn find_file(&mut self, path: &str) -> Result<Rc<dyn Body>, PackageError> {
        self.files
            .file(path)
            .map_err(|source| PackageError::Filesystem {
                partition: self.partition.clone(),
                source,
            })?
            .ok_or_else(|| PackageError::FileMissing {
                partition: self.partition.clone(),
                path: path.to_owned(),
            })
    }

    fn check_digest(
        &self,
        path: &str,
        expected: Sha256Digest,
        found: Sha256Digest,
    ) -> Result<(), PackageError> {
        if found != expected {
            return Err(PackageError::FileMismatch {
                partition: self.partition.clone(),
                path: path.to_owned(),
                expected,
                found,
            });
        }
        Ok(())
    }

    #[must_use]
    fn read_failure(&self, path: &str, source: SourceError) -> PackageError {
        PackageError::FileRead {
            partition: self.partition.clone(),
            path: path.to_owned(),
            source,
        }
    }
}
