// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::progress::ShownPercent;

pub(crate) struct ArrivedBytes(Rc<RefCell<ArrivalCount>>);

struct ArrivalCount {
    done: u64,
    total: u64,
    shown: ShownPercent,
    report: Box<dyn Fn(u64)>,
}

impl ArrivedBytes {
    pub(crate) fn start(done: u64, total: u64, report: impl Fn(u64) + 'static) -> Self {
        Self(Rc::new(RefCell::new(ArrivalCount {
            done,
            total,
            shown: ShownPercent::default(),
            report: Box::new(report),
        })))
    }

    pub(crate) fn file(&self, size: u64) -> FileArrival {
        self.0.borrow_mut().report_new_percent();
        FileArrival {
            count: Rc::clone(&self.0),
            counted: Rc::default(),
            size,
        }
    }

    pub(crate) fn done(&self) -> u64 {
        self.0.borrow().done
    }
}

impl ArrivalCount {
    fn recount(&mut self, before: u64, after: u64) {
        self.done = self.done - before + after;
        self.report_new_percent();
    }

    fn report_new_percent(&mut self) {
        if self.shown.advances_to(self.done, self.total) {
            (self.report)(self.done);
        }
    }
}

#[derive(Clone)]
pub(crate) struct FileArrival {
    count: Rc<RefCell<ArrivalCount>>,
    counted: Rc<Cell<u64>>,
    size: u64,
}

impl FileArrival {
    pub(crate) fn restart(&self) {
        self.recount(0);
    }

    pub(crate) fn add(&self, len: u64) {
        self.recount(self.counted.get().saturating_add(len).min(self.size));
    }

    fn recount(&self, counted: u64) {
        let before = self.counted.replace(counted);
        self.count.borrow_mut().recount(before, counted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_try_takes_back_the_bytes_of_the_failed_one_and_a_file_counts_up_to_its_size() {
        let reports = Rc::new(RefCell::new(Vec::new()));
        let shown = Rc::clone(&reports);
        let arrived = ArrivedBytes::start(100, 300, move |done| shown.borrow_mut().push(done));
        let file = arrived.file(200);
        file.add(150);
        file.restart();
        file.add(250);
        assert_eq!(*reports.borrow(), [100, 250, 100, 300]);
    }
}
