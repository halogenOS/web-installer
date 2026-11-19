// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::io::Write;
use std::num::NonZeroUsize;
use std::rc::Rc;

use byte_io::{Body, ByteSource, CopyError, Scratch, SourceError, visit_pieces};
use fs_read::ImageFiles;
use sha256::{RunningDigest, Sha256Digest};

use crate::{CopyFileError, PackageError};

pub struct PartitionFiles {
    partition: String,
    files: ImageFiles,
    scratch: Scratch,
}

impl PartitionFiles {
    /// Opens the filesystem in `image`, the image of the partition `partition`. Files are read
    /// through a buffer of `piece_len` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError::Filesystem`] when the filesystem of `image` cannot be read.
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

    /// Writes the file `path` to `sink` and returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// Returns [`CopyFileError::Package`] when the function cannot search the filesystem for
    /// `path`, finds no file `path`, cannot read the file or computes a SHA-256 other than `sha256`
    /// for it. Returns [`CopyFileError::Sink`] when `sink` fails. On any error, `sink` may already
    /// contain part or all of the file.
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
