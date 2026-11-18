// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::rc::Rc;

use byte_io::{ByteSource, SourceError};
use fastboot::{Fastboot, InstallHost, InstallProgress, check_phone};
use release::PartitionName;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::host_link::{LinkFailure, PageLink};
use crate::host_source::HostSource;
use crate::install_failure::InstallError;
use crate::page::{InstallPage, OpenFailure, Page};
use crate::phone_check_failure::PhoneUnfit;
use crate::phone_check_report::PhoneCheckReport;
use crate::progress::{Progress, ReleasePlan};
use crate::release_images::{ImagesFor, StoredName, store_checked_images};
use crate::releases::read_release_of_tag;

#[wasm_bindgen]
pub async fn install(page: InstallPage, release_tag: String) -> Result<(), JsValue> {
    run(&Page::new(page), &release_tag)
        .await
        .map_err(|failure| failure.to_page_failure())
}

async fn run(page: &Page, release_tag: &str) -> Result<(), InstallError> {
    page.report(&Progress::Connecting);
    let messages = page.clone();
    let mut fastboot = Fastboot::new(
        page.open_phone().await?,
        Box::new(move |text| messages.show_phone_message(text)),
    );
    let (published, release) = read_release_of_tag(page.site(), release_tag).await?;
    page.report(&Progress::Checking(ReleasePlan::of(
        &release,
        ImagesFor::Install,
    )));
    let phone = check_phone(&mut fastboot, &release, &mut |check| {
        page.report(&Progress::PhoneChecked(PhoneCheckReport::of(check)));
    })
    .await?
    .map_err(|failure| {
        InstallError::PhoneUnfit(PhoneUnfit {
            device: release.parts().device.clone(),
            failure,
        })
    })?;
    store_checked_images(page, &release, &published, ImagesFor::Install, None).await?;
    let mut host = StoredImages { page: page.clone() };
    fastboot::install(&release, &phone, fastboot, &mut host).await?;
    page.remove_all_files()
        .await
        .map_err(InstallError::FilesKept)?;
    page.report(&Progress::Done);
    Ok(())
}

struct StoredImages {
    page: Page,
}

impl StoredImages {
    fn open(&self, name: &str) -> Result<Rc<dyn ByteSource>, SourceError> {
        Ok(Rc::new(HostSource::open(self.page.file(name)?)?))
    }
}

impl InstallHost for StoredImages {
    type Link = PageLink;

    fn image(&self, partition: &PartitionName) -> Result<Rc<dyn ByteSource>, SourceError> {
        self.open(&StoredName::Image(partition).to_string())
    }

    fn super_empty(&self) -> Result<Rc<dyn ByteSource>, SourceError> {
        self.open(&StoredName::SuperEmpty.to_string())
    }

    async fn reconnect(&mut self) -> Result<PageLink, LinkFailure> {
        self.page
            .reopen_phone()
            .await
            .map_err(|failure| match failure {
                OpenFailure::Unavailable(problem) => LinkFailure::Unavailable(problem),
                OpenFailure::Page(failure) => LinkFailure::Transfer(failure),
            })
    }

    fn report(&mut self, progress: InstallProgress<'_>) {
        self.page.report(&match progress {
            InstallProgress::Step { index, count, step } => {
                Progress::Installing { index, count, step }
            }
            InstallProgress::Sent { sent, total } => Progress::Sending { done: sent, total },
        });
    }
}
