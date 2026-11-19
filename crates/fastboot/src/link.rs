// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::error::Error;
use std::future::Future;

pub trait Link {
    type Error: Error + 'static;

    /// The most bytes one `send` hands to the transport. A download body longer than this goes
    /// out in pieces of this length, and only its last piece is shorter. fastbootd ends its read
    /// of a body at the first USB packet shorter than the packet size
    /// (`ClientUsbTransport::Read` in `system/core/fastboot/device/usb_client.cpp` of
    /// android-17.0.0_r1, checked 2026-11-19), so this length is a whole number of packets.
    const SEND_LIMIT: usize;

    /// Sends `bytes` to the phone in one transfer of at most [`Self::SEND_LIMIT`] bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when the transfer fails.
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;

    /// Receives one transfer of 1 to `limit` bytes from the phone.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] when the transfer fails or does not have 1 to `limit` bytes.
    fn receive(&mut self, limit: u32) -> impl Future<Output = Result<Vec<u8>, Self::Error>>;
}
