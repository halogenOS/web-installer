// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::rc::Rc;

use byte_io::clamp_len;
use image_archive::{ArchiveError, ForwardWalk};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::arrived_bytes::FileArrival;

#[wasm_bindgen]
pub struct RangeArrival(Rc<RefCell<RangeCount>>);

struct RangeCount {
    len: u64,
    taken: u64,
    arrival: FileArrival,
    walk: Option<WalkOverRange>,
    failure: Option<ArchiveError>,
}

struct WalkOverRange {
    walk: Rc<RefCell<ForwardWalk>>,
    at_start: ForwardWalk,
}

pub(crate) enum RangeEnd {
    Complete,
    EndedEarly { received: u64 },
}

#[wasm_bindgen]
impl RangeArrival {
    pub fn restart(&self) {
        let mut count = self.0.borrow_mut();
        count.taken = 0;
        count.failure = None;
        count.arrival.restart();
        if let Some(walk) = &count.walk {
            *walk.walk.borrow_mut() = walk.at_start.clone();
        }
    }

    #[must_use]
    pub fn take(&self, piece: &[u8]) -> usize {
        let mut count = self.0.borrow_mut();
        let piece = &piece[..clamp_len(count.len - count.taken, piece.len())];
        let taken = match &count.walk {
            Some(walk) => walk.walk.borrow_mut().take(piece),
            None => Ok(piece.len()),
        };
        let taken = taken.unwrap_or_else(|failure| {
            count.failure = Some(failure);
            0
        });
        count.taken += taken as u64;
        count.arrival.add(taken as u64);
        taken
    }
}

impl RangeArrival {
    pub(crate) fn new(
        len: u64,
        arrival: FileArrival,
        walk: Option<&Rc<RefCell<ForwardWalk>>>,
    ) -> Self {
        Self(Rc::new(RefCell::new(RangeCount {
            len,
            taken: 0,
            arrival,
            walk: walk.map(|walk| WalkOverRange {
                at_start: walk.borrow().clone(),
                walk: Rc::clone(walk),
            }),
            failure: None,
        })))
    }

    pub(crate) fn share(&self) -> Self {
        Self(Rc::clone(&self.0))
    }

    pub(crate) fn end(&self) -> Result<RangeEnd, ArchiveError> {
        let mut count = self.0.borrow_mut();
        if let Some(failure) = count.failure.take() {
            return Err(failure);
        }
        let walked = count
            .walk
            .as_ref()
            .is_some_and(|walk| walk.walk.borrow().is_complete());
        Ok(if walked || count.taken == count.len {
            RangeEnd::Complete
        } else {
            RangeEnd::EndedEarly {
                received: count.taken,
            }
        })
    }
}
