// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::future::Future;

pub trait Link {
    type Error: Error + 'static;

    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;

    fn receive(&mut self, limit: u32) -> impl Future<Output = Result<Vec<u8>, Self::Error>>;
}
