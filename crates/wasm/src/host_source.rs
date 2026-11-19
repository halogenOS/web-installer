// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use byte_io::{ByteSource, SourceError, to_len};
use js_sys::Uint8Array;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

use crate::host::{HostFailure, read_from_page};
use crate::number::integer_of;

#[wasm_bindgen]
extern "C" {
    type PageFile;

    #[wasm_bindgen(method, catch)]
    fn size(this: &PageFile) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn read(this: &PageFile, offset: f64, length: f64) -> Result<JsValue, JsValue>;
}

pub(crate) struct HostSource {
    file: PageFile,
    len: u64,
}

impl HostSource {
    pub(crate) fn open(object: JsValue) -> Result<Self, SourceError> {
        Self::connect(object.unchecked_into()).map_err(SourceError::from)
    }

    fn connect(file: PageFile) -> Result<Self, HostFailure> {
        let len = integer_of(number_from("size", file.size())?)?;
        Ok(Self { file, len })
    }

    fn page_bytes(&self, offset: u64, len: u64) -> Result<Uint8Array, SourceError> {
        read_from_page(|start, length| self.file.read(start, length), offset, len)
    }
}

impl ByteSource for HostSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), SourceError> {
        let len = to_len(buf.len());
        self.ensure_range(offset, len)?;
        self.page_bytes(offset, len)?.copy_to(buf);
        Ok(())
    }

    fn read_up_to(&self, offset: u64, len: u64) -> Result<Vec<u8>, SourceError> {
        self.ensure_range(offset, len)?;
        Ok(self.page_bytes(offset, len)?.to_vec())
    }
}

fn number_from(name: &str, returned: Result<JsValue, JsValue>) -> Result<f64, HostFailure> {
    returned
        .map_err(|thrown| HostFailure::of_exception(&thrown))?
        .as_f64()
        .ok_or_else(|| HostFailure::contract_violation(format!("{name} must return a number")))
}
