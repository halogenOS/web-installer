// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;

use byte_io::{SourceError, TransferFault};
use js_sys::{Function, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};

use crate::number::{NumberError, number_of};
use crate::retry::TransferFailure;

const CONTRACT_VIOLATION: &str = "HostContractViolation";

const UNNAMED_FAILURE: &str = "Error";

const QUOTA_EXCEEDED: &str = "QuotaExceededError";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostFailure {
    name: String,
    message: String,
}

impl HostFailure {
    pub(crate) fn contract_violation(message: impl Into<String>) -> Self {
        Self::of_parts(Some(CONTRACT_VIOLATION.to_owned()), Some(message.into()))
    }

    pub(crate) fn of_exception(thrown: &JsValue) -> Self {
        let field = |key: &str| {
            Reflect::get(thrown, &JsValue::from_str(key))
                .ok()
                .and_then(|value| value.as_string())
        };
        Self::of_parts(
            field("name"),
            field("message").or_else(|| thrown.as_string()),
        )
    }

    pub(crate) fn is_storage_full(&self) -> bool {
        self.name == QUOTA_EXCEEDED
    }

    pub(crate) fn to_error(&self) -> js_sys::Error {
        let error = js_sys::Error::new(&self.message);
        error.set_name(&self.name);
        error
    }

    fn of_parts(name: Option<String>, message: Option<String>) -> Self {
        Self {
            name: name.unwrap_or_else(|| UNNAMED_FAILURE.to_owned()),
            message: message.unwrap_or_default(),
        }
    }
}

impl Display for HostFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.message)
    }
}

impl Error for HostFailure {}

impl From<JsValue> for HostFailure {
    fn from(thrown: JsValue) -> Self {
        Self::of_exception(&thrown)
    }
}

impl From<NumberError> for HostFailure {
    fn from(error: NumberError) -> Self {
        Self::contract_violation(error.to_string())
    }
}

impl From<HostFailure> for SourceError {
    fn from(failure: HostFailure) -> Self {
        Self::Host {
            name: failure.name,
            message: failure.message,
        }
    }
}

impl From<HostFailure> for io::Error {
    fn from(failure: HostFailure) -> Self {
        let kind = if failure.is_storage_full() {
            io::ErrorKind::QuotaExceeded
        } else {
            io::ErrorKind::Other
        };
        Self::new(kind, failure)
    }
}

pub(crate) fn host_function(object: &JsValue, name: &str) -> Result<Function, HostFailure> {
    Reflect::get(object, &JsValue::from_str(name))
        .map_err(|thrown| HostFailure::of_exception(&thrown))?
        .dyn_into::<Function>()
        .map_err(|_| HostFailure::contract_violation(format!("{name} must be a function")))
}

pub(crate) fn page_number(value: u64) -> Result<JsValue, HostFailure> {
    Ok(JsValue::from_f64(number_of(value)?))
}

pub(crate) fn read_from_page(
    call: impl FnOnce(f64, f64) -> Result<JsValue, JsValue>,
    offset: u64,
    len: u64,
) -> Result<Uint8Array, SourceError> {
    let returned = call(
        number_of(offset).map_err(HostFailure::from)?,
        number_of(len).map_err(HostFailure::from)?,
    )
    .map_err(|thrown| read_failure(&thrown))?;
    let bytes = returned
        .dyn_into::<Uint8Array>()
        .map_err(|_| HostFailure::contract_violation("read must return a Uint8Array"))?;
    ensure_delivered(len, bytes.length())?;
    Ok(bytes)
}

fn read_failure(thrown: &JsValue) -> SourceError {
    Reflect::get(thrown, &JsValue::from_str("failure"))
        .ok()
        .and_then(|failure| serde_wasm_bindgen::from_value::<TransferFailure>(failure).ok())
        .map_or_else(
            || HostFailure::of_exception(thrown).into(),
            |failure| SourceError::Transfer {
                fault: TransferFault::Unavailable,
                source: io::Error::other(failure),
            },
        )
}

fn ensure_delivered(requested: u64, delivered: u32) -> Result<(), HostFailure> {
    if u64::from(delivered) == requested {
        Ok(())
    } else {
        Err(HostFailure::contract_violation(format!(
            "read returned {delivered} bytes where {requested} were requested"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quota_failure() -> HostFailure {
        HostFailure::of_parts(
            Some(QUOTA_EXCEEDED.to_owned()),
            Some("no room left".to_owned()),
        )
    }

    #[test]
    fn a_failure_of_the_page_keeps_its_name_and_message_as_a_source_error() {
        let error = SourceError::from(quota_failure());

        assert!(
            matches!(
                &error,
                SourceError::Host { name, message }
                    if name == QUOTA_EXCEEDED && message == "no room left"
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_failure_of_the_page_keeps_its_name_and_message_as_an_io_error() {
        let error = io::Error::from(quota_failure());

        assert_eq!(error.to_string(), "QuotaExceededError: no room left");
        assert_eq!(
            error
                .get_ref()
                .and_then(|cause| cause.downcast_ref::<HostFailure>()),
            Some(&quota_failure())
        );
    }

    #[test]
    fn a_storage_without_room_is_an_io_error_of_the_kind_quota_exceeded() {
        let error = io::Error::from(quota_failure());

        assert_eq!(error.kind(), io::ErrorKind::QuotaExceeded);
    }

    #[test]
    fn any_other_failure_of_the_page_is_an_io_error_of_the_kind_other() {
        let not_readable = HostFailure::of_parts(Some("NotReadableError".to_owned()), None);

        let error = io::Error::from(not_readable);

        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn a_thrown_value_without_a_name_or_a_message_is_still_a_failure() {
        assert_eq!(
            HostFailure::of_parts(None, None).to_string(),
            format!("{UNNAMED_FAILURE}: ")
        );
    }

    #[test]
    fn a_broken_contract_has_the_name_of_a_contract_violation() {
        let error = SourceError::from(HostFailure::contract_violation("read must answer"));

        assert!(
            matches!(
                &error,
                SourceError::Host { name, message }
                    if name == CONTRACT_VIOLATION && message == "read must answer"
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_refused_number_is_a_broken_contract_that_says_which_number() {
        let error = SourceError::from(HostFailure::from(NumberError::NotAnExactInteger(1.5)));

        let SourceError::Host { name, message } = error else {
            panic!("expected a failure of the host, got {error:?}");
        };
        assert_eq!(name, CONTRACT_VIOLATION);
        assert!(message.contains("1.5"), "{message}");
    }

    #[test]
    fn a_read_that_delivers_the_requested_length_is_accepted() {
        assert_eq!(ensure_delivered(4096, 4096), Ok(()));
        assert_eq!(ensure_delivered(0, 0), Ok(()));
    }

    #[test]
    fn a_read_that_delivers_another_length_is_a_broken_contract() {
        for delivered in [0, 4095, 4097] {
            let failure = ensure_delivered(4096, delivered).unwrap_err();
            let text = failure.to_string();
            assert!(
                text.contains(&delivered.to_string()) && text.contains("4096"),
                "{text}"
            );
        }
    }
}
