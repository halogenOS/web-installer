// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use ota_package::operation_data;
use release::ChunkOperationKind;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host::HostFailure;

#[wasm_bindgen(js_name = compressChunk)]
pub fn compress_chunk(kind: JsValue, chunk: Vec<u8>) -> Result<Vec<u8>, JsValue> {
    let kind: ChunkOperationKind = serde_wasm_bindgen::from_value(kind).map_err(|error| {
        JsValue::from(HostFailure::contract_violation(error.to_string()).to_error())
    })?;
    Ok(operation_data(kind, chunk))
}
