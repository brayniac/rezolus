use super::*;
use crate::agent::*;

use libbpf_rs::Map;
use memmap2::{MmapMut, MmapOptions};
use metriken::LazyGauge;

use std::os::fd::{AsFd, AsRawFd, FromRawFd};

/// This wraps the BPF map along with an opened memory-mapped region for the map
/// values.
struct GaugeMap<'a> {
    _map: &'a Map<'a>,
    mmap: MmapMut,
    bank_width: usize,
}

impl<'a> GaugeMap<'a> {
    /// Create a new `GaugeMap` from the provided BPF map that holds the
    /// provided number of gauges.
    pub fn new(map: &'a Map, gauges: usize) -> Result<Self, ()> {
        // each CPU has its own bank of gauges, this bank is the next nearest
        // whole number of cachelines wide
        let bank_cachelines = whole_cachelines::<u64>(gauges);

        // the number of possible slots per bank of gauges
        let bank_width = bank_cachelines * GAUGES_PER_CACHELINE;

        // our total mapped region size in bytes
        let total_bytes = bank_cachelines * CACHELINE_SIZE * MAX_CPUS;

        let fd = map.as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let mmap: MmapMut = unsafe {
            MmapOptions::new()
                .len(total_bytes)
                .map_mut(&file)
                .map_err(|e| error!("failed to mmap() bpf GaugeMap: {e}"))
        }?;

        let (_prefix, values, _suffix) = unsafe { mmap.align_to::<u64>() };

        if values.len() != MAX_CPUS * bank_width {
            error!("mmap region not aligned or width doesn't match");
            return Err(());
        }

        Ok(Self {
            _map: map,
            mmap,
            bank_width,
        })
    }

    /// Borrow a reference to the raw values.
    pub fn values(&self) -> &[u64] {
        let (_prefix, values, _suffix) = unsafe { self.mmap.align_to::<u64>() };
        values
    }

    /// Get the bank width which is the stride for reading through the values
    /// slice.
    pub fn bank_width(&self) -> usize {
        self.bank_width
    }
}

/// Represents a set of gauges where the BPF map is a dense set of gauges,
/// meaning there is no padding. No aggregation is performed, and the values are
/// updated into a single `RwLockGaugeGroup`.
pub struct PackedGauges<'a> {
    _map: &'a Map<'a>,
    mmap: MmapMut,
    gauges: &'static GaugeGroup,
}

impl<'a> PackedGauges<'a> {
    /// Create a new set of gauges from the provided BPF map and collection of
    /// counter metrics.
    ///
    /// The map layout is not cacheline padded. The ordering of the dynamic
    /// gauges must exactly match the layout in the BPF map.
    pub fn new(map: &'a Map, gauges: &'static GaugeGroup) -> Self {
        let total_bytes = gauges.len() * std::mem::size_of::<u64>();

        let fd = map.as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let mmap: MmapMut = unsafe {
            MmapOptions::new()
                .len(total_bytes)
                .map_mut(&file)
                .expect("failed to mmap() bpf GaugeGroup")
        };

        let (_prefix, values, _suffix) = unsafe { mmap.align_to::<u64>() };

        if values.len() != gauges.len() {
            panic!("mmap region not aligned or width doesn't match");
        }

        Self {
            _map: map,
            mmap,
            gauges,
        }
    }

    /// Refreshes the gauges by reading from the BPF map and setting each
    /// counter metric to the current value.
    pub fn refresh(&mut self) {
        let (_prefix, values, _suffix) = unsafe { self.mmap.align_to::<u64>() };

        // update all individual gauges
        for (idx, value) in values.iter().enumerate() {
            if *value != 0 {
                let _ = self.gauges.set(idx, *value);
            }
        }
    }
}
