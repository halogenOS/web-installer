// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::future::Future;
use std::rc::Rc;

use byte_io::{ByteSource, SourceError};
use release::{PartitionName, Release, RequiredVariable, Step};

use crate::protocol::Command;
use crate::{Fastboot, FastbootError, Link};

const YES: &str = "yes";
const NO_SNAPSHOT_UPDATE: &str = "none";

pub trait InstallHost {
    type Link: Link;

    fn image(&self, partition: &PartitionName) -> Result<Rc<dyn ByteSource>, SourceError>;

    fn super_empty(&self) -> Result<Rc<dyn ByteSource>, SourceError>;

    fn reconnect(
        &mut self,
    ) -> impl Future<Output = Result<Self::Link, <Self::Link as Link>::Error>>;

    fn report(&mut self, progress: InstallProgress<'_>);
}

#[derive(Clone, Copy, Debug)]
pub enum InstallProgress<'a> {
    Step {
        index: usize,
        count: usize,
        step: &'a Step,
    },
    Sent {
        sent: u64,
        total: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnmetRequirement {
    pub variable: RequiredVariable,
    pub reported: Option<String>,
    pub values: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhoneCheck {
    pub unmet: Vec<UnmetRequirement>,
    pub unlocked: bool,
}

pub async fn check_phone<L: Link>(
    fastboot: &mut Fastboot<L>,
    release: &Release,
) -> Result<PhoneCheck, FastbootError<L::Error>> {
    let mut unmet = Vec::new();
    for requirement in &release.parts().requirements {
        let reported = known_variable(fastboot, requirement.variable.getvar_name()).await?;
        if !reported
            .as_ref()
            .is_some_and(|value| requirement.values.contains(value))
        {
            unmet.push(UnmetRequirement {
                variable: requirement.variable,
                reported,
                values: requirement.values.clone(),
            });
        }
    }
    let unlocked = known_variable(fastboot, "unlocked").await?.as_deref() == Some(YES);
    Ok(PhoneCheck { unmet, unlocked })
}

async fn known_variable<L: Link>(
    fastboot: &mut Fastboot<L>,
    name: &str,
) -> Result<Option<String>, FastbootError<L::Error>> {
    match fastboot.getvar(name).await {
        Ok(value) => Ok(Some(value)),
        Err(FastbootError::Failed { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

#[must_use]
pub fn install_steps(release: &Release) -> Vec<&Step> {
    release
        .parts()
        .install
        .iter()
        .filter(|step| !matches!(step, Step::Erase { .. }))
        .collect()
}

pub async fn install<H: InstallHost>(
    release: &Release,
    mut fastboot: Fastboot<H::Link>,
    host: &mut H,
) -> Result<(), FastbootError<<H::Link as Link>::Error>> {
    let slot = fastboot.getvar("current-slot").await?;
    let slot = slot.trim_start_matches('_').to_owned();
    let steps = install_steps(release);
    for (index, &step) in steps.iter().enumerate() {
        host.report(InstallProgress::Step {
            index,
            count: steps.len(),
            step,
        });
        match step {
            Step::Flash { partition, logical } => {
                let target = if release.parts().is_slotted(partition) {
                    format!("{partition}_{slot}")
                } else {
                    partition.to_string()
                };
                let image = host.image(partition).map_err(FastbootError::Read)?;
                let total = image.len();
                if *logical {
                    fastboot
                        .run(Command::ResizeLogicalPartition {
                            partition: &target,
                            size: total,
                        })
                        .await?;
                }
                let max_download = max_download_size(&mut fastboot).await?;
                let mut sent = 0;
                let mut on_sent = |bytes| {
                    sent += bytes;
                    host.report(InstallProgress::Sent { sent, total });
                };
                fastboot
                    .flash(&target, &*image, max_download, &mut on_sent)
                    .await?;
            }
            Step::RebootFastboot => {
                fastboot.run(Command::RebootFastboot).await?;
                let link = host.reconnect().await.map_err(FastbootError::Link)?;
                fastboot = fastboot.reconnected(link);
                let is_userspace = fastboot.getvar("is-userspace").await?;
                if is_userspace != YES {
                    return Err(FastbootError::NotFastbootd { is_userspace });
                }
            }
            Step::UpdateSuper { super_device, .. } => {
                if let Some(status) =
                    known_variable(&mut fastboot, "snapshot-update-status").await?
                    && !status.is_empty()
                    && status != NO_SNAPSHOT_UPDATE
                {
                    fastboot.run(Command::CancelSnapshotUpdate).await?;
                }
                let layout = host.super_empty().map_err(FastbootError::Read)?;
                fastboot
                    .download_whole(&*layout, &mut |_layout_bytes| {})
                    .await?;
                fastboot
                    .run(Command::UpdateSuper(super_device.as_str()))
                    .await?;
            }
            Step::Erase { .. } => {}
        }
    }
    fastboot.run(Command::Reboot).await
}

async fn max_download_size<L: Link>(
    fastboot: &mut Fastboot<L>,
) -> Result<u64, FastbootError<L::Error>> {
    const NAME: &str = "max-download-size";
    let value = fastboot.getvar(NAME).await?;
    match value.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => value.parse(),
    }
    .map_err(|_| FastbootError::UnexpectedReply {
        command: format!("getvar:{NAME}"),
        reply: value,
    })
}
