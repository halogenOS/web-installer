// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

#![forbid(unsafe_code)]

use std::{error::Error, fmt, time::Duration};

use http::StatusCode;
use serde::Deserialize;

const FIRST_PAUSE: Duration = Duration::from_secs(1);

pub const LONGEST_PAUSE: Duration = Duration::from_mins(1);

const TEMPORARY_STATUSES: [StatusCode; 7] = [
    StatusCode::REQUEST_TIMEOUT,
    StatusCode::TOO_EARLY,
    StatusCode::TOO_MANY_REQUESTS,
    StatusCode::INTERNAL_SERVER_ERROR,
    StatusCode::BAD_GATEWAY,
    StatusCode::SERVICE_UNAVAILABLE,
    StatusCode::GATEWAY_TIMEOUT,
];

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TransferFailure {
    Connection,
    CrossOriginRefused,
    Status {
        status: u16,
        retry_after: Option<String>,
    },
}

impl fmt::Display for TransferFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status { status, .. } => write!(f, "the host returned status {status}"),
            Self::CrossOriginRefused => f.write_str("the host does not let this page read it"),
            Self::Connection => f.write_str("the connection failed"),
        }
    }
}

impl Error for TransferFailure {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pause {
    Exactly(Duration),
    UpTo(Duration),
}

impl Pause {
    #[must_use]
    pub fn pick(self, spread: f64) -> Duration {
        match self {
            Self::Exactly(time) => time,
            Self::UpTo(longest) => longest.mul_f64(spread.clamp(0.0, 1.0)),
        }
    }
}

#[must_use]
pub fn pause_before_retry(failure: &TransferFailure, attempt: u32) -> Option<Pause> {
    let growing = Pause::UpTo(
        FIRST_PAUSE
            .saturating_mul(
                1_u32
                    .checked_shl(attempt.saturating_sub(1))
                    .unwrap_or(u32::MAX),
            )
            .min(LONGEST_PAUSE),
    );
    match failure {
        TransferFailure::Connection => Some(growing),
        TransferFailure::Status {
            status,
            retry_after,
        } if is_temporary(*status) => Some(
            retry_after
                .as_deref()
                .and_then(|text| text.trim().parse().ok())
                .map_or(growing, |seconds| {
                    Pause::Exactly(Duration::from_secs(seconds))
                }),
        ),
        TransferFailure::CrossOriginRefused | TransferFailure::Status { .. } => None,
    }
}

fn is_temporary(status: u16) -> bool {
    StatusCode::from_u16(status).is_ok_and(|status| TEMPORARY_STATUSES.contains(&status))
}
