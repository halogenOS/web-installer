// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::rc::Rc;

use byte_io::{ByteSource, SourceError, clamp_len};
use update_metadata::PartitionUpdate;
use update_metadata::install_operation::Type;

use crate::PackageError;
use crate::operation::{Operation, Storage};

#[derive(Clone, Copy, Debug)]
struct Run {
    len: u64,
    operation: usize,
    skip: u64,
}

struct FilledBlocks {
    operation: usize,
    bytes: Vec<u8>,
}

enum Piece {
    Run {
        operation: usize,
        skip: u64,
        len: usize,
    },
    Zeros {
        len: usize,
    },
}

pub(crate) struct PartitionImage {
    payload: Rc<dyn ByteSource>,
    size: u64,
    operations: Vec<Operation>,
    runs: BTreeMap<u64, Run>,
    data: RefCell<BTreeMap<usize, Rc<[u8]>>>,
    last: RefCell<Option<FilledBlocks>>,
}

impl PartitionImage {
    pub(crate) fn new(
        payload: Rc<dyn ByteSource>,
        data_start: u64,
        block_size: u64,
        update: &PartitionUpdate,
    ) -> Result<Self, PackageError> {
        let partition = &update.partition_name;
        let size = update
            .new_partition_info
            .as_ref()
            .and_then(|info| info.size)
            .ok_or_else(|| PackageError::PartitionSizeMissing {
                partition: partition.clone(),
            })?;
        let mut image = Self {
            payload,
            size,
            operations: Vec::with_capacity(update.operations.len()),
            runs: BTreeMap::new(),
            data: RefCell::new(BTreeMap::new()),
            last: RefCell::new(None),
        };
        for operation in &update.operations {
            let storage = Type::try_from(operation.r#type)
                .ok()
                .and_then(Storage::of)
                .ok_or_else(|| PackageError::UnsupportedOperation {
                    partition: partition.clone(),
                    operation: operation.r#type,
                })?;
            let index = image.operations.len();
            let mut filled_len = 0;
            for extent in &operation.dst_extents {
                let (start_block, num_blocks) = (extent.start_block(), extent.num_blocks());
                let out_of_range = || PackageError::ExtentOutOfRange {
                    partition: partition.clone(),
                    start_block,
                    num_blocks,
                };
                let start = start_block
                    .checked_mul(block_size)
                    .ok_or_else(out_of_range)?;
                let len = num_blocks
                    .checked_mul(block_size)
                    .ok_or_else(out_of_range)?;
                if !image.is_free(start, len) {
                    return Err(out_of_range());
                }
                if len > 0 {
                    let _new_run = image.runs.insert(
                        start,
                        Run {
                            len,
                            operation: index,
                            skip: filled_len,
                        },
                    );
                }
                filled_len += len;
            }
            let (offset, length) = (operation.data_offset(), operation.data_length());
            let data = data_start
                .checked_add(offset)
                .and_then(|start| Some(start..start.checked_add(length)?))
                .filter(|data| data.end <= image.payload.len())
                .filter(|_| length <= filled_len)
                .ok_or_else(|| PackageError::DataOutOfRange {
                    partition: partition.clone(),
                    offset,
                    length,
                })?;
            image.operations.push(Operation {
                storage,
                data,
                sha256: operation.data_sha256_hash.clone().ok_or_else(|| {
                    PackageError::OperationHashMissing {
                        partition: partition.clone(),
                    }
                })?,
                filled_len,
                block_size,
            });
        }
        Ok(image)
    }

    pub(crate) fn largest_operation_len(&self) -> NonZeroUsize {
        let largest = self
            .operations
            .iter()
            .map(|operation| operation.filled_len)
            .max()
            .unwrap_or(0);
        NonZeroUsize::new(clamp_len(largest, usize::MAX)).unwrap_or(NonZeroUsize::MIN)
    }

    fn is_free(&self, start: u64, len: u64) -> bool {
        let Some(end) = start.checked_add(len).filter(|end| *end <= self.size) else {
            return false;
        };
        let before = self.runs.range(..end).next_back();
        before.is_none_or(|(run_start, run)| run_start + run.len <= start)
    }

    fn piece_at(&self, at: u64, limit: usize) -> Piece {
        let covering = self
            .runs
            .range(..=at)
            .next_back()
            .filter(|(start, run)| at < *start + run.len);
        if let Some((start, run)) = covering {
            Piece::Run {
                operation: run.operation,
                skip: run.skip + (at - start),
                len: clamp_len(start + run.len - at, limit),
            }
        } else {
            let next = self
                .runs
                .range(at..)
                .next()
                .map_or(self.size, |(start, _)| *start);
            Piece::Zeros {
                len: clamp_len(next - at, limit),
            }
        }
    }

    fn read_run(&self, operation: usize, skip: u64, out: &mut [u8]) -> Result<(), SourceError> {
        let mut last = self.last.borrow_mut();
        let filled = match last.take() {
            Some(filled) if filled.operation == operation => filled,
            _ => FilledBlocks {
                operation,
                bytes: self.operations[operation].fill_blocks(&self.data_of(operation)?)?,
            },
        };
        let start = usize::try_from(skip).expect("a run lies in the bytes of its operation");
        out.copy_from_slice(&filled.bytes[start..start + out.len()]);
        *last = Some(filled);
        Ok(())
    }

    fn data_of(&self, operation: usize) -> Result<Rc<[u8]>, SourceError> {
        if let Some(data) = self.data.borrow().get(&operation) {
            return Ok(Rc::clone(data));
        }
        let data: Rc<[u8]> = self.operations[operation].read_data(&self.payload)?.into();
        let _first_read = self.data.borrow_mut().insert(operation, Rc::clone(&data));
        Ok(data)
    }
}

impl ByteSource for PartitionImage {
    fn len(&self) -> u64 {
        self.size
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), SourceError> {
        self.ensure_range(offset, buf.len() as u64)?;
        let mut done = 0;
        while done < buf.len() {
            let at = offset + done as u64;
            let rest = &mut buf[done..];
            let piece = match self.piece_at(at, rest.len()) {
                Piece::Run {
                    operation,
                    skip,
                    len,
                } => {
                    self.read_run(operation, skip, &mut rest[..len])?;
                    len
                }
                Piece::Zeros { len } => {
                    rest[..len].fill(0);
                    len
                }
            };
            done += piece;
        }
        Ok(())
    }

    fn read_up_to(&self, offset: u64, len: u64) -> Result<Vec<u8>, SourceError> {
        self.ensure_range(offset, len)?;
        let mut bytes = Vec::new();
        while (bytes.len() as u64) < len {
            let at = offset + bytes.len() as u64;
            let piece = match self.piece_at(at, clamp_len(len - bytes.len() as u64, usize::MAX)) {
                Piece::Run { len, .. } | Piece::Zeros { len } => len,
            };
            let start = bytes.len();
            bytes.resize(start + piece, 0);
            self.read_at(at, &mut bytes[start..])?;
        }
        Ok(bytes)
    }
}
