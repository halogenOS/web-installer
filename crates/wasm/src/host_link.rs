// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use fastboot::Link;
use js_sys::{Function, Promise, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::host::{HostFailure, host_function};
use crate::page::PhoneProblem;

#[derive(Debug)]
pub(crate) enum LinkFailure {
    Transfer(HostFailure),
    Contract(HostFailure),
    Unavailable(PhoneProblem),
}

impl LinkFailure {
    fn contract_violation(message: impl Into<String>) -> Self {
        Self::Contract(HostFailure::contract_violation(message))
    }
}

impl Display for LinkFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transfer(source) | Self::Contract(source) => Display::fmt(source, f),
            Self::Unavailable(problem) => Display::fmt(problem, f),
        }
    }
}

impl Error for LinkFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transfer(source) | Self::Contract(source) => Some(source),
            Self::Unavailable(_) => None,
        }
    }
}

pub(crate) struct PageLink {
    object: JsValue,
    send_function: Function,
    receive_function: Function,
}

impl PageLink {
    pub(crate) fn open(object: JsValue) -> Result<Self, HostFailure> {
        let send_function = host_function(&object, "send")?;
        let receive_function = host_function(&object, "receive")?;
        Ok(Self {
            object,
            send_function,
            receive_function,
        })
    }

    async fn call(&self, function: &Function, argument: &JsValue) -> Result<JsValue, HostFailure> {
        let result = function
            .call1(&self.object, argument)
            .map_err(HostFailure::from)?;
        JsFuture::from(Promise::resolve(&result))
            .await
            .map_err(HostFailure::from)
    }
}

impl Link for PageLink {
    type Error = LinkFailure;

    async fn send(&mut self, bytes: &[u8]) -> Result<(), LinkFailure> {
        let _result = self
            .call(&self.send_function, &Uint8Array::from(bytes))
            .await
            .map_err(LinkFailure::Transfer)?;
        Ok(())
    }

    async fn receive(&mut self, limit: u32) -> Result<Vec<u8>, LinkFailure> {
        let result = self
            .call(&self.receive_function, &JsValue::from(limit))
            .await
            .map_err(LinkFailure::Transfer)?;
        let bytes = result
            .dyn_into::<Uint8Array>()
            .map_err(|_| LinkFailure::contract_violation("receive must return a Uint8Array"))?;
        let length = bytes.length();
        if length == 0 || length > limit {
            return Err(LinkFailure::contract_violation(format!(
                "receive returned {length} bytes, outside the range of 1 to {limit}"
            )));
        }
        Ok(bytes.to_vec())
    }
}
