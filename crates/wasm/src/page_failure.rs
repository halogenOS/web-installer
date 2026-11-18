// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::fmt::Display;

use failure_text::cause_sentence;
use serde::Serialize;
use wasm_bindgen::JsValue;

pub(crate) const REPORT_DEFECT: &str = "This is a defect of the installer. Report this text to us.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageFailure {
    cause: String,
    next_step: &'static str,
}

impl PageFailure {
    pub(crate) fn new(cause: impl Display, next_step: &'static str) -> Self {
        Self {
            cause: cause_sentence(cause),
            next_step,
        }
    }
}

pub(crate) fn page_failure(cause: impl Display, next_step: &'static str) -> JsValue {
    serde_wasm_bindgen::to_value(&PageFailure::new(cause, next_step)).unwrap_or_else(Into::into)
}
