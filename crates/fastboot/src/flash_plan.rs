// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use release::{PartitionName, Release, Step};

#[must_use]
pub fn install_steps(release: &Release) -> Vec<&Step> {
    release
        .parts()
        .install
        .iter()
        .filter(|step| !matches!(step, Step::Erase { .. }))
        .collect()
}

pub(crate) fn flash_target(release: &Release, partition: &PartitionName, slot: &str) -> String {
    if release.parts().is_slotted(partition) {
        format!("{partition}_{slot}")
    } else {
        partition.to_string()
    }
}
