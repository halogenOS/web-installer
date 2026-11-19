// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::io::Write;
use std::rc::Rc;

use byte_io::ByteSource;
use sha256::Sha256Digest;

use crate::{CopyFileError, PackageError, PartitionFiles, StockPackage};

pub struct PackageFiles {
    package: StockPackage,
    partitions: BTreeMap<String, PartitionFiles>,
}

impl PackageFiles {
    /// Opens the stock OTA package `package`.
    ///
    /// # Errors
    ///
    /// Returns the [`PackageError`] for the first part of `package` that the function cannot read
    /// or that does not belong to a full stock OTA package. The function reads the zip archive,
    /// then the `payload.bin` entry, which the archive must store uncompressed, then the payload
    /// header and the manifest.
    pub fn open(package: Rc<dyn ByteSource>) -> Result<Self, PackageError> {
        Ok(Self {
            package: StockPackage::open(package)?,
            partitions: BTreeMap::new(),
        })
    }

    /// Writes the file `path` of the partition `partition` to `sink` and returns the number of
    /// bytes written.
    ///
    /// # Errors
    ///
    /// Returns [`CopyFileError::Package`] when the function cannot read the image of `partition`
    /// from the payload or the filesystem in that image. Otherwise it returns the error of
    /// [`PartitionFiles::copy_file`], and `sink` may then already contain part or all of the file.
    pub fn copy_file(
        &mut self,
        partition: &str,
        path: &str,
        sha256: Sha256Digest,
        sink: &mut dyn Write,
    ) -> Result<u64, CopyFileError> {
        self.partition(partition)?.copy_file(path, sha256, sink)
    }

    pub fn close_partitions(&mut self) {
        self.partitions.clear();
    }

    fn partition(&mut self, partition: &str) -> Result<&mut PartitionFiles, PackageError> {
        Ok(match self.partitions.entry(partition.to_owned()) {
            Entry::Occupied(opened) => opened.into_mut(),
            Entry::Vacant(entry) => entry.insert(self.package.partition_files(partition)?),
        })
    }
}
