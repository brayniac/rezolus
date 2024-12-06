#![allow(dead_code)]
#![allow(unused_imports)]

use metriken::Metric;
use metriken::Value;
use parking_lot::RwLock;
use std::sync::OnceLock;
use thiserror::Error;

mod dynamic;
mod scoped;

pub use dynamic::{DynamicCounter, DynamicCounterBuilder};
pub use scoped::ScopedCounters;

pub const MAX_CGROUPS: usize = 4 * 1024 * 1024;

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("the index is higher than the counter group size")]
    InvalidIndex,
}

/// A group of counters that's protected by a reader-writer lock.
pub struct RwLockCounterGroup {
    inner: OnceLock<RwLock<Vec<u64>>>,
    entries: usize,
}

impl Metric for RwLockCounterGroup {
    fn as_any(&self) -> std::option::Option<&(dyn std::any::Any + 'static)> {
        Some(self)
    }

    fn value(&self) -> std::option::Option<metriken::Value<'_>> {
        Some(Value::Other(self))
    }
}

impl RwLockCounterGroup {
    /// Create a new counter group
    pub const fn new(entries: usize) -> Self {
        Self {
            inner: OnceLock::new(),
            entries,
        }
    }

    /// Sets the counter at a given index to the provided value
    pub fn set(&self, idx: usize, value: u64) -> Result<(), Error> {
        if idx >= self.entries {
            Err(Error::InvalidIndex)
        } else {
            let mut inner = self.get_or_init().write();

            inner[idx] = value;

            Ok(())
        }
    }

    /// Load the counter values
    pub fn load(&self) -> Option<Vec<u64>> {
        self.inner.get().map(|v| v.read().clone())
    }

    pub fn len(&self) -> usize {
        self.entries
    }

    fn get_or_init(&self) -> &RwLock<Vec<u64>> {
        self.inner.get_or_init(|| vec![0; self.entries].into())
    }
}
