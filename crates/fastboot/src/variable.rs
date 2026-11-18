// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::fmt;
use std::str::FromStr;

use release::RequiredVariable;

use crate::protocol::Command;
use crate::{Fastboot, FastbootError, Link};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Variable {
    Required(RequiredVariable),
    Unlocked,
    BatterySocOk,
    CurrentSlot,
    PartitionSize(String),
    MaxDownloadSize,
    IsUserspace,
    SnapshotUpdateStatus,
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Required(variable) => f.write_str(variable.getvar_name()),
            Self::Unlocked => f.write_str("unlocked"),
            Self::BatterySocOk => f.write_str("battery-soc-ok"),
            Self::CurrentSlot => f.write_str("current-slot"),
            Self::PartitionSize(partition) => write!(f, "partition-size:{partition}"),
            Self::MaxDownloadSize => f.write_str("max-download-size"),
            Self::IsUserspace => f.write_str("is-userspace"),
            Self::SnapshotUpdateStatus => f.write_str("snapshot-update-status"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unreported {
    pub variable: Variable,
    pub refusal: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum YesNo {
    Yes,
    No,
}

impl FromStr for YesNo {
    type Err = ();

    fn from_str(answer: &str) -> Result<Self, Self::Err> {
        match answer {
            "yes" => Ok(Self::Yes),
            "no" => Ok(Self::No),
            _ => Err(()),
        }
    }
}

impl<L: Link> Fastboot<L> {
    pub(crate) async fn read_variable(
        &mut self,
        variable: &Variable,
    ) -> Result<Result<String, Unreported>, FastbootError<L::Error>> {
        let refusal = match self.getvar(variable).await {
            Ok(value) if !value.is_empty() => return Ok(Ok(value)),
            Ok(_) => None,
            Err(FastbootError::Failed { message, .. }) => Some(message),
            Err(error) => return Err(error),
        };
        Ok(Err(Unreported {
            variable: variable.clone(),
            refusal,
        }))
    }

    pub(crate) async fn read_answer(
        &mut self,
        variable: &Variable,
    ) -> Result<Result<YesNo, Unreported>, FastbootError<L::Error>> {
        match self.read_variable(variable).await? {
            Ok(answer) => answer
                .parse()
                .map(Ok)
                .map_err(|()| unexpected_reply(variable, answer)),
            Err(unreported) => Ok(Err(unreported)),
        }
    }

    pub(crate) async fn read_size(
        &mut self,
        variable: &Variable,
    ) -> Result<Result<u64, Unreported>, FastbootError<L::Error>> {
        match self.read_variable(variable).await? {
            Ok(value) => parse_size(variable, value).map(Ok),
            Err(unreported) => Ok(Err(unreported)),
        }
    }

    pub(crate) async fn require_size(
        &mut self,
        variable: &Variable,
    ) -> Result<u64, FastbootError<L::Error>> {
        let value = self.getvar(variable).await?;
        parse_size(variable, value)
    }
}

fn parse_size<E>(variable: &Variable, value: String) -> Result<u64, FastbootError<E>> {
    match value.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => value.parse(),
    }
    .map_err(|_| unexpected_reply(variable, value))
}

fn unexpected_reply<E>(variable: &Variable, reply: String) -> FastbootError<E> {
    FastbootError::UnexpectedReply {
        command: Command::GetVar(variable).text(),
        reply,
    }
}
