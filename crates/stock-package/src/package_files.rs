// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::io::Write;
use std::rc::Rc;

use byte_io::{Body, ByteSource};
use sha256::Sha256Digest;

use crate::{CopyFileError, PackageError, PartitionFiles, StockPackage};

pub struct PackageFiles {
    package: StockPackage,
    partitions: BTreeMap<String, PartitionFiles>,
}

impl PackageFiles {
    pub fn open(package: Rc<dyn ByteSource>) -> Result<Self, PackageError> {
        Ok(Self {
            package: StockPackage::open(package)?,
            partitions: BTreeMap::new(),
        })
    }

    pub fn file_with_digest(
        &mut self,
        partition: &str,
        path: &str,
    ) -> Result<(Rc<dyn Body>, Sha256Digest), PackageError> {
        self.partition(partition)?.file_with_digest(path)
    }

    pub fn file_bytes(
        &mut self,
        partition: &str,
        path: &str,
        sha256: Sha256Digest,
    ) -> Result<Vec<u8>, PackageError> {
        self.partition(partition)?.file_bytes(path, sha256)
    }

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
