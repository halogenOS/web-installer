// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

pub(crate) use transfer_retry::TransferFailure;
use transfer_retry::pause_before_retry;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host::HostFailure;

const LONGEST_TIMER_MILLIS: u32 = 2_147_483_647;

#[wasm_bindgen(js_name = pauseBeforeRetry)]
pub fn pause_before_retry_ms(failure: JsValue, attempt: u32) -> Result<Option<f64>, JsValue> {
    let failure: TransferFailure = serde_wasm_bindgen::from_value(failure)
        .map_err(|error| HostFailure::contract_violation(error.to_string()).to_error())?;
    Ok(pause_before_retry_millis(&failure, attempt))
}

pub(crate) fn pause_before_retry_millis(failure: &TransferFailure, attempt: u32) -> Option<f64> {
    pause_before_retry(failure, attempt).map(|pause| {
        let millis = pause.pick(js_sys::Math::random()).as_millis();
        f64::from(
            u32::try_from(millis).map_or(LONGEST_TIMER_MILLIS, |millis| {
                millis.min(LONGEST_TIMER_MILLIS)
            }),
        )
    })
}
