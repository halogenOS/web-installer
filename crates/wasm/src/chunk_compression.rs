// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use ota_package::operation_data;
use release::ChunkOperationKind;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host::HostFailure;

/// Returns the data of a payload operation that stores `chunk` as `kind` sets it: as it is, or
/// compressed with bzip2 or xz.
///
/// # Errors
///
/// Fails with a [host contract violation](crate#failures) when `kind` is not a chunk operation
/// kind, and with a [compression failure](crate#failures) when the compressor fails on `chunk`.
#[wasm_bindgen(js_name = compressChunk)]
pub fn compress_chunk(kind: JsValue, chunk: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    let kind: ChunkOperationKind = serde_wasm_bindgen::from_value(kind)
        .map_err(|error| HostFailure::contract_violation(error.to_string()).to_error())?;
    let data = operation_data(kind, chunk)
        .map_err(|error| HostFailure::compression_failed(error.to_string()).to_error())?;
    Ok(data)
}
