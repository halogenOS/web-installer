// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use sparse_image::{
    BLOCK_LEN, CHUNK_HEADER_LEN, ChunkHeader, ChunkType, FILE_HEADER_LEN, FileHeader, SparseLayout,
};

const MAX_CHUNKS: u64 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Segment {
    Bytes(Vec<u8>),
    Image { start: u64, len: u64 },
}

pub(crate) fn downloads(len: u64, max_download: u64) -> Option<Vec<Vec<Segment>>> {
    if len <= max_download {
        return Some(vec![vec![Segment::Image { start: 0, len }]]);
    }
    let block_len = u64::from(BLOCK_LEN);
    let total_blocks = u32::try_from(len.div_ceil(block_len)).ok()?;
    let headers = u64::from(FILE_HEADER_LEN) + MAX_CHUNKS * u64::from(CHUNK_HEADER_LEN);
    let blocks_per_download =
        (max_download.checked_sub(headers)? / block_len).min(u64::from(u32::MAX));
    let blocks_per_download = u32::try_from(blocks_per_download)
        .ok()
        .filter(|blocks| *blocks > 0)?;
    Some(
        (0..total_blocks)
            .step_by(usize::try_from(blocks_per_download).ok()?)
            .map(|first| {
                let count = blocks_per_download.min(total_blocks - first);
                piece(len, total_blocks, first, count)
            })
            .collect(),
    )
}

fn piece(len: u64, total_blocks: u32, first: u32, count: u32) -> Vec<Segment> {
    let start = u64::from(first) * u64::from(BLOCK_LEN);
    let data_len = (u64::from(count) * u64::from(BLOCK_LEN)).min(len - start);
    let after = total_blocks - first - count;
    let file = FileHeader::new(SparseLayout {
        block_len: BLOCK_LEN,
        total_blocks,
        total_chunks: 1 + u32::from(first > 0) + u32::from(after > 0),
    });
    let chunk_header = |kind, blocks| {
        ChunkHeader::new(kind, blocks, &file)
            .expect("a download is shorter than 4 GiB")
            .to_bytes()
    };
    let mut segments = vec![Segment::Bytes(file.to_bytes())];
    if first > 0 {
        segments.push(Segment::Bytes(chunk_header(ChunkType::DontCare, first)));
    }
    segments.push(Segment::Bytes(chunk_header(ChunkType::Raw, count)));
    segments.push(Segment::Image {
        start,
        len: data_len,
    });
    let padding = u64::from(count) * u64::from(BLOCK_LEN) - data_len;
    if padding > 0 {
        segments.push(Segment::Bytes(vec![
            0;
            usize::try_from(padding).expect(
                "the rest of a block fits in memory"
            )
        ]));
    }
    if after > 0 {
        segments.push(Segment::Bytes(chunk_header(ChunkType::DontCare, after)));
    }
    segments
}
