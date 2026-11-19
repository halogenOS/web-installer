// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use sparse_image::{BLOCK_LEN, ChunkHeader, ChunkType, FileHeader, SparseLayout};

pub(crate) fn blocks_len(blocks: u32) -> u64 {
    u64::from(blocks) * u64::from(BLOCK_LEN)
}

/// Returns the bytes of `blocks` raw blocks, with every byte set to `fill`.
pub(crate) fn filled_blocks(blocks: u32, fill: u8) -> Vec<u8> {
    vec![fill; usize::try_from(blocks_len(blocks)).unwrap()]
}

/// A sparse image of `raw_blocks` raw blocks, then `skipped_blocks` skipped blocks.
pub(crate) fn sparse_image(raw_blocks: u32, skipped_blocks: u32) -> Vec<u8> {
    let file = FileHeader::new(SparseLayout {
        block_len: BLOCK_LEN,
        total_blocks: raw_blocks + skipped_blocks,
        total_chunks: 2,
    });
    let chunk = |kind, blocks| {
        ChunkHeader::new(kind, blocks, BLOCK_LEN)
            .unwrap()
            .to_bytes()
    };
    [
        file.to_bytes(),
        chunk(ChunkType::Raw, raw_blocks),
        filled_blocks(raw_blocks, 7),
        chunk(ChunkType::DontCare, skipped_blocks),
    ]
    .concat()
}
