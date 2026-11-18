// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

#![forbid(unsafe_code)]

mod archive_ranges;
mod arrived_bytes;
mod carrier;
mod chunk_compression;
mod file_digest;
mod github_releases;
mod host;
mod host_link;
mod host_sink;
mod host_source;
mod image_archive_files;
mod install;
mod install_failure;
mod kept_images;
mod number;
mod package;
mod package_failure;
mod page;
mod page_failure;
mod picked_files;
mod picked_release_files;
mod progress;
mod range_arrival;
mod release_fetches;
mod release_files_failure;
mod release_images;
mod releases;
mod remote_file;
mod retry;
mod stock_files;

pub use chunk_compression::compress_chunk;
pub use file_digest::FileDigest;
pub use install::install;
pub use install_failure::phone_problem_failure;
pub use package::save_package;
pub use package_failure::save_dialog_failure;
pub use page::{InstallPage, PackagePage, ReleaseSite, StoragePage};
pub use range_arrival::RangeArrival;
pub use releases::list_releases;
pub use retry::pause_before_retry_ms;
