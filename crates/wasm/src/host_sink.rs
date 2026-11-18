// SPDX-License-Identifier: GPL-3.0-only

use std::io::{self, Write};

use js_sys::Function;
use wasm_bindgen::JsValue;

use crate::carrier::Carrier;
use crate::host::{HostFailure, host_function};
use crate::number::integer_of;

pub(crate) struct HostSink {
    object: JsValue,
    write: Function,
    flush: Function,
    carrier: Carrier,
}

impl HostSink {
    pub(crate) fn open(object: JsValue) -> Result<Self, HostFailure> {
        let write = host_function(&object, "write")?;
        let flush = host_function(&object, "flush")?;
        Ok(Self {
            object,
            write,
            flush,
            carrier: Carrier::new(),
        })
    }
}

impl Write for HostSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let chunk = JsValue::from(self.carrier.carry(bytes)?);
        let answer: JsValue = self
            .write
            .call1(&self.object, &chunk)
            .map_err(|thrown| HostFailure::of_exception(&thrown))?;
        Ok(accepted_count(answer.as_f64(), bytes.len())?)
    }

    fn flush(&mut self) -> io::Result<()> {
        let _flush_answers_nothing: JsValue = self
            .flush
            .call0(&self.object)
            .map_err(|thrown| HostFailure::of_exception(&thrown))?;
        Ok(())
    }
}

fn accepted_count(answer: Option<f64>, offered: usize) -> Result<usize, HostFailure> {
    let number = answer.ok_or_else(|| {
        HostFailure::contract_violation("write must answer with the number of bytes it took")
    })?;
    let accepted = integer_of(number)?;
    usize::try_from(accepted)
        .ok()
        .filter(|&count| count <= offered)
        .ok_or_else(|| {
            HostFailure::contract_violation(format!(
                "write took {accepted} bytes where {offered} were offered"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bytes_the_page_took_are_what_it_answered() {
        assert_eq!(accepted_count(Some(4096.0), 4096).unwrap(), 4096);
        assert_eq!(accepted_count(Some(10.0), 4096).unwrap(), 10);
        assert_eq!(accepted_count(Some(0.0), 0).unwrap(), 0);
    }

    #[test]
    fn a_page_that_took_nothing_is_left_to_the_caller_to_refuse() {
        assert_eq!(accepted_count(Some(0.0), 4096).unwrap(), 0);
    }

    #[test]
    fn an_answer_that_is_not_a_count_of_the_offered_bytes_is_a_broken_contract() {
        for answer in [None, Some(4097.0), Some(-1.0), Some(1.5), Some(f64::NAN)] {
            assert!(accepted_count(answer, 4096).is_err(), "{answer:?}");
        }
    }

    #[test]
    fn a_count_above_the_offer_names_both_numbers() {
        let text = accepted_count(Some(4097.0), 4096).unwrap_err().to_string();
        assert!(text.contains("4097") && text.contains("4096"), "{text}");
    }
}
