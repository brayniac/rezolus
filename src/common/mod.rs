#![allow(dead_code)]

#[cfg(all(feature = "bpf", target_os = "linux"))]
pub mod bpf;

pub mod classic;
pub mod units;

mod counter;
mod interval;
mod nop;

use std::borrow::Borrow;
use parking_lot::Condvar;
use parking_lot::Mutex;

pub use clocksource::precise::UnixInstant;
pub use counter::Counter;
pub use interval::{AsyncInterval, Interval};
pub use nop::Nop;

pub const HISTOGRAM_GROUPING_POWER: u8 = 7;

use tokio::sync::Notify;

pub struct SyncPrimitive {
    trigger: (Mutex<bool>, Condvar),
    notify: Notify,
}

impl SyncPrimitive {
    pub fn new() -> Self {
        Self {
            trigger: (Mutex::new(false), Condvar::new()),
            notify: Notify::new(),
        }
    }

    pub fn trigger(&self) {
        let (ref lock, ref cvar) = self.trigger.borrow();
        let mut started = lock.lock();
        *started = true;
        cvar.notify_one();
    }

    pub fn wait_for_trigger(&self) {
        let (ref lock, ref cvar) = self.trigger.borrow();
        let mut started = lock.lock();
        if !*started {
            cvar.wait(&mut started);
        }
        *started = false;
    }

    pub fn notify(&self) {
        self.notify.notify_waiters();
    }

    pub async fn wait_for_notify(&self) {
        self.notify.notified().await;
    }
}

#[cfg(test)]
mod tests {
    use crate::common::SyncPrimitive;

    #[test]
    fn sync_primitive() {
        let sync = SyncPrimitive::new();

        sync.trigger();
        sync.wait_for_trigger();
    }
}