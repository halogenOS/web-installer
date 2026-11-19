// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::rc::Rc;

use byte_io::to_len;
use sha256::{RunningDigest, Sha256Digest};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::arrived_bytes::FileArrival;

#[wasm_bindgen]
pub struct FileDigest {
    digest: Rc<RefCell<RunningDigest>>,
    arrival: FileArrival,
}

#[wasm_bindgen]
impl FileDigest {
    pub fn restart(&self) {
        *self.digest.borrow_mut() = RunningDigest::default();
        self.arrival.restart();
    }

    pub fn update(&self, piece: &[u8]) {
        self.digest.borrow_mut().update(piece);
        self.arrival.add(to_len(piece.len()));
    }
}

impl FileDigest {
    pub(crate) fn new(arrival: FileArrival) -> Self {
        Self {
            digest: Rc::default(),
            arrival,
        }
    }

    pub(crate) fn share(&self) -> Self {
        Self {
            digest: Rc::clone(&self.digest),
            arrival: self.arrival.clone(),
        }
    }

    pub(crate) fn digest(&self) -> Sha256Digest {
        self.digest.borrow().clone().finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrived_bytes::ArrivedBytes;

    fn digest_of_file(len: usize) -> FileDigest {
        let len = to_len(len);
        FileDigest::new(ArrivedBytes::start(0, len, |_| {}).file(len))
    }

    fn digest_in_pieces(bytes: &[u8], piece_len: usize) -> Sha256Digest {
        let digest = digest_of_file(bytes.len());
        let page = digest.share();
        for piece in bytes.chunks(piece_len) {
            page.update(piece);
        }
        digest.digest()
    }

    #[test]
    fn a_file_handed_in_as_pieces_has_the_digest_of_its_whole_bytes() {
        let file = b"vendor partition ".repeat(4096);
        for piece_len in [1, 777, file.len()] {
            assert_eq!(
                digest_in_pieces(&file, piece_len),
                Sha256Digest::of(&file),
                "{piece_len}"
            );
        }
    }

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn a_file_of_no_bytes_has_the_digest_of_no_bytes() {
        assert_eq!(digest_of_file(0).digest().to_string(), EMPTY_SHA256);
    }
}
