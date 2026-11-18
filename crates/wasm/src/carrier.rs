// SPDX-License-Identifier: GPL-3.0-only

use std::io;

use js_sys::Uint8Array;

pub(crate) struct Carrier(Uint8Array);

impl Carrier {
    pub(crate) fn new() -> Self {
        Self(Uint8Array::new_with_length(0))
    }

    pub(crate) fn carry(&mut self, bytes: &[u8]) -> io::Result<Uint8Array> {
        let len =
            u32::try_from(bytes.len()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        if self.0.length() < len {
            self.0 = Uint8Array::new_with_length(len);
        }
        let view = self.0.subarray(0, len);
        view.copy_from(bytes);
        Ok(view)
    }
}
