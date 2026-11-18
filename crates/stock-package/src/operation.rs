// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::io::{self, Read};
use std::ops::Range;
use std::rc::Rc;

use byte_io::{ByteSource, Digest, SourceError};
use sha256::Sha256Digest;
use update_metadata::install_operation::Type;

use crate::read_range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Storage {
    Raw,
    Bzip2,
    Xz,
}

impl Storage {
    pub(crate) fn of(kind: Type) -> Option<Self> {
        match kind {
            Type::Replace => Some(Self::Raw),
            Type::ReplaceBz => Some(Self::Bzip2),
            Type::ReplaceXz => Some(Self::Xz),
            Type::Move
            | Type::Bsdiff
            | Type::SourceCopy
            | Type::SourceBsdiff
            | Type::Zero
            | Type::Discard
            | Type::Puffdiff
            | Type::BrotliBsdiff
            | Type::Zucchini
            | Type::Lz4diffBsdiff
            | Type::Lz4diffPuffdiff
            | Type::ReplaceZstd => None,
        }
    }

    fn decode(self, data: &[u8], limit: u64) -> io::Result<Vec<u8>> {
        let limit = limit.saturating_add(1);
        let mut bytes = Vec::new();
        let _read: usize = match self {
            Self::Raw => data.take(limit).read_to_end(&mut bytes),
            Self::Bzip2 => bzip2_rs::DecoderReader::new(data)
                .take(limit)
                .read_to_end(&mut bytes),
            Self::Xz => lzma_rust2::XzReader::new(data, false)
                .take(limit)
                .read_to_end(&mut bytes),
        }?;
        Ok(bytes)
    }
}

pub(crate) struct Operation {
    pub storage: Storage,
    pub data: Range<u64>,
    pub sha256: Vec<u8>,
    pub filled_len: u64,
    pub block_size: u64,
}

impl Operation {
    pub fn fill_blocks(&self, data: &[u8]) -> Result<Vec<u8>, SourceError> {
        let mut bytes = self
            .storage
            .decode(data, self.filled_len)
            .map_err(|source| SourceError::Decode {
                range: self.data.clone(),
                source,
            })?;
        let len = bytes.len() as u64;
        if len > self.filled_len {
            return Err(SourceError::BodyExceedsLength {
                declared: self.filled_len,
            });
        }
        if len.next_multiple_of(self.block_size) < self.filled_len {
            return Err(SourceError::BodyEndedEarly {
                declared: self.filled_len,
                delivered: len,
            });
        }
        let padding = usize::try_from(self.filled_len - len).expect("less than one block");
        bytes.resize(bytes.len() + padding, 0);
        Ok(bytes)
    }

    pub fn read_data(&self, payload: &Rc<dyn ByteSource>) -> Result<Vec<u8>, SourceError> {
        let data = read_range(payload, self.data.start, self.data.end - self.data.start)?;
        let found = Sha256Digest::of(&data);
        if found.as_bytes() != self.sha256.as_slice() {
            return Err(SourceError::Integrity {
                range: self.data.clone(),
                expected: Digest::new(self.sha256.clone()),
                found: Digest::new(found.as_bytes().to_vec()),
            });
        }
        Ok(data)
    }
}
