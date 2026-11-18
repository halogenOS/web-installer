// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::fmt::{self, Display, Formatter};

const MAX_EXACT: u64 = (1 << 53) - 1;

const UPPER_HALF_WEIGHT: f64 = 4_294_967_296.0;

const SIGN_BIT: u64 = 1 << 63;
const FRACTION_BITS: u64 = 52;
const FRACTION_MASK: u64 = (1 << FRACTION_BITS) - 1;
const EXPONENT_BIAS: u64 = 1023;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NumberError {
    BeyondExactRange(u64),
    NotAnExactInteger(f64),
}

impl Display for NumberError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeyondExactRange(value) => write!(
                f,
                "{value} is larger than {MAX_EXACT}, the largest integer that a number holds exactly"
            ),
            Self::NotAnExactInteger(number) => {
                write!(f, "{number} is not a whole number from 0 up to {MAX_EXACT}")
            }
        }
    }
}

impl Error for NumberError {}

pub(crate) fn number_of(value: u64) -> Result<f64, NumberError> {
    if value > MAX_EXACT {
        return Err(NumberError::BeyondExactRange(value));
    }
    let half = |bits: u64| {
        u32::try_from(bits)
            .map(f64::from)
            .map_err(|_| NumberError::BeyondExactRange(value))
    };
    Ok(half(value >> 32)? * UPPER_HALF_WEIGHT + half(value & u64::from(u32::MAX))?)
}

pub(crate) fn integer_of(number: f64) -> Result<u64, NumberError> {
    let bits = number.to_bits();
    let magnitude = bits & !SIGN_BIT;
    if magnitude == 0 {
        return Ok(0);
    }
    let refused = NumberError::NotAnExactInteger(number);
    if bits & SIGN_BIT != 0 {
        return Err(refused);
    }
    let exponent = (magnitude >> FRACTION_BITS)
        .checked_sub(EXPONENT_BIAS)
        .filter(|&exponent| exponent <= FRACTION_BITS)
        .ok_or(refused)?;
    let significand = (1 << FRACTION_BITS) | (magnitude & FRACTION_MASK);
    let fraction_bits = FRACTION_BITS - exponent;
    if significand & ((1 << fraction_bits) - 1) != 0 {
        return Err(refused);
    }
    Ok(significand >> fraction_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_gives_back_the_integer_it_was_made_of() {
        for value in [0, 1, 4095, u64::from(u32::MAX), 1 << 32, 1 << 52, MAX_EXACT] {
            let number = number_of(value).unwrap();
            assert_eq!(integer_of(number), Ok(value), "{value}");
        }
    }

    #[test]
    fn an_integer_beyond_the_exact_range_is_refused() {
        for value in [MAX_EXACT + 1, u64::MAX] {
            assert_eq!(number_of(value), Err(NumberError::BeyondExactRange(value)));
        }
    }

    #[test]
    fn the_numbers_of_whole_values_are_read_as_integers() {
        assert_eq!(integer_of(0.0), Ok(0));
        assert_eq!(integer_of(-0.0), Ok(0));
        assert_eq!(integer_of(1.0), Ok(1));
        assert_eq!(integer_of(1_048_576.0), Ok(1 << 20));
        assert_eq!(integer_of(4_294_967_296.0), Ok(1 << 32));
        assert_eq!(integer_of(9_007_199_254_740_991.0), Ok(MAX_EXACT));
    }

    #[test]
    fn a_number_that_is_not_a_whole_value_from_zero_to_the_exact_range_is_refused() {
        for number in [
            9_007_199_254_740_992.0,
            1.0e300,
            0.5,
            1.5,
            4_294_967_296.25,
            -1.0,
            f64::MIN_POSITIVE,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert!(
                matches!(integer_of(number), Err(NumberError::NotAnExactInteger(_))),
                "{number}"
            );
        }
    }

    #[test]
    fn a_refusal_names_the_value_and_the_range() {
        let text = integer_of(1.5).unwrap_err().to_string();
        assert!(
            text.contains("1.5") && text.contains("9007199254740991"),
            "{text}"
        );
        let text = number_of(u64::MAX).unwrap_err().to_string();
        assert!(text.contains(&u64::MAX.to_string()), "{text}");
    }
}
