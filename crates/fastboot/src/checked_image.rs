// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::rc::Rc;

use byte_io::ByteSource;
use sparse_image::{ImageBlocks, SparseImageError, read_sparse_file};

/// An image that the install read before it writes the image to a partition.
pub(crate) struct CheckedImage {
    image: Rc<dyn ByteSource>,
    layout: ImageLayout,
}

enum ImageLayout {
    Raw,
    Sparse {
        blocks: ImageBlocks,
        contents_len: u64,
    },
}

/// The downloads that send an image to a partition, each one flashed after it arrives.
pub(crate) struct ImageDownloads {
    downloads: Vec<Rc<dyn ByteSource>>,
}

impl CheckedImage {
    pub(crate) fn read(image: Rc<dyn ByteSource>) -> Result<Self, SparseImageError> {
        let layout = match read_sparse_file(&*image)? {
            Some(file) => ImageLayout::Sparse {
                contents_len: file.contents_len(),
                blocks: ImageBlocks::of_sparse(Rc::clone(&image), &file),
            },
            None => ImageLayout::Raw,
        };
        Ok(Self { image, layout })
    }

    pub(crate) fn len(&self) -> u64 {
        self.image.len()
    }

    /// The bytes that the image writes to its partition.
    pub(crate) fn partition_len(&self) -> u64 {
        match &self.layout {
            ImageLayout::Raw => self.image.len(),
            ImageLayout::Sparse { contents_len, .. } => *contents_len,
        }
    }

    /// The image whole when it fits one download of `max_download` bytes, otherwise split into
    /// sparse images that do. `None` when a download of `max_download` bytes is too short for one
    /// block of the image.
    pub(crate) fn plan(&self, max_download: u32) -> Option<ImageDownloads> {
        if self.image.len() <= u64::from(max_download) {
            return Some(ImageDownloads {
                downloads: vec![Rc::clone(&self.image)],
            });
        }
        let parts = match &self.layout {
            ImageLayout::Raw => ImageBlocks::of_raw(Rc::clone(&self.image))?.split(max_download),
            ImageLayout::Sparse { blocks, .. } => blocks.split(max_download),
        }?;
        Some(ImageDownloads {
            downloads: parts
                .into_iter()
                .map(|part| -> Rc<dyn ByteSource> { Rc::new(part) })
                .collect(),
        })
    }
}

impl ImageDownloads {
    pub(crate) fn bodies(&self) -> &[Rc<dyn ByteSource>] {
        &self.downloads
    }

    pub(crate) fn sent_len(&self) -> u64 {
        self.downloads.iter().map(|download| download.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use byte_io::{MemorySource, to_index, to_len};
    use sparse_image::{BLOCK_LEN, min_split_len};

    use super::*;
    use crate::test_images::{blocks_len, sparse_image};

    fn checked(bytes: Vec<u8>) -> CheckedImage {
        CheckedImage::read(Rc::new(MemorySource::new(bytes))).unwrap()
    }

    fn whole_download(downloads: &ImageDownloads) -> bool {
        matches!(downloads.bodies(), [body] if body.len() == downloads.sent_len())
    }

    #[test]
    fn a_raw_image_writes_its_own_length_to_the_partition() {
        let image = checked(vec![1; 100]);
        assert!(matches!(image.layout, ImageLayout::Raw));
        assert_eq!(image.partition_len(), 100);
    }

    #[test]
    fn a_sparse_image_writes_the_blocks_it_describes_to_the_partition() {
        assert_eq!(checked(sparse_image(2, 8)).partition_len(), blocks_len(10));
    }

    #[test]
    fn an_image_that_fits_one_download_goes_out_whole() {
        for bytes in [vec![1; 100], sparse_image(2, 8)] {
            let len = to_len(bytes.len());
            let downloads = checked(bytes).plan(u32::try_from(len).unwrap()).unwrap();
            assert!(whole_download(&downloads));
            assert_eq!(downloads.sent_len(), len);
        }
    }

    #[test]
    fn a_longer_raw_image_goes_out_as_sparse_images() {
        let raw = checked(vec![1; to_index(blocks_len(3))]);
        let max_download = u32::try_from(min_split_len(BLOCK_LEN)).unwrap();
        assert_eq!(raw.plan(max_download).unwrap().bodies().len(), 3);
    }

    #[test]
    fn a_download_too_short_for_one_block_has_no_plan() {
        let max_download = u32::try_from(min_split_len(BLOCK_LEN) - 1).unwrap();
        assert!(checked(sparse_image(2, 8)).plan(max_download).is_none());
    }
}
