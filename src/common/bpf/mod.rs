// Allow dead code for now
#![allow(dead_code)]

use super::*;
use core::time::Duration;
use metriken::DynBoxedMetric;
use metriken::RwLockHistogram;
use ouroboros::*;
use std::os::fd::{AsFd, AsRawFd, FromRawFd};
use std::sync::Arc;

pub use libbpf_rs::skel::{OpenSkel, Skel, SkelBuilder};
pub use libbpf_rs::OpenObject;
pub use std::mem::MaybeUninit;

mod counters;
mod histogram;

use counters::BpfCounters;
use histogram::BpfHistogram;

pub use counters::PercpuCounters;

const PAGE_SIZE: usize = 4096;
const CACHELINE_SIZE: usize = 64;

/// The maximum number of CPUs supported. Allows a normal bpf map behave like
/// per-CPU counters by packing counters into cacheline sized chunks such that
/// no CPUs will share cacheline sized segments of the counter map.
static MAX_CPUS: usize = 1024;

/// Returns the next nearest whole number of pages that fits a histogram with
/// the provided config.
pub fn histogram_pages(buckets: usize) -> usize {
    ((buckets * std::mem::size_of::<u64>()) + PAGE_SIZE - 1) / PAGE_SIZE
}

/// A trait that must be implemented to assist in getting a reference to a named
/// BPF map.
pub trait GetMap {
    fn map(&self, name: &str) -> &libbpf_rs::Map;
}

/// This is a builder type that is used to configure and register all BPF maps
/// during initialization. The `Bpf` type returned will prevent runtime
/// registration of additional maps.
///
/// The 'static lifetime bound is required for ouroboros self-referencing type.
pub struct BpfBuilder<T: 'static> {
    bpf: _Bpf<T>,
}

impl<T: 'static + GetMap> BpfBuilder<T> {
    pub fn new(skel: T) -> Self {
        Self {
            bpf: _Bpf::from_skel(skel),
        }
    }

    pub fn build(self) -> Bpf<T> {
        Bpf { bpf: self.bpf }
    }

    pub fn counters(mut self, name: &str, counters: Vec<CounterWithHist>) -> Self {
        self.bpf = self.bpf.add_counters(name, counters);
        self
    }

    pub fn percpu_counters(
        mut self,
        name: &str,
        counters: Vec<CounterWithHist>,
        percpu: Arc<PercpuCounters>,
    ) -> Self {
        self.bpf = self.bpf.add_counters_with_percpu(name, counters, percpu);
        self
    }

    pub fn histogram(mut self, name: &str, histogram: &'static RwLockHistogram) -> Self {
        self.bpf = self.bpf.add_histogram(name, histogram);
        self
    }

    pub fn map(self, name: &str, values: &[u64]) -> Self {
        let fd = self.bpf.map(name).as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let mut mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(std::mem::size_of_val(values))
                .map_mut(&file)
                .expect("failed to mmap() bpf map")
        };

        for (index, bytes) in mmap
            .chunks_exact_mut(std::mem::size_of::<u64>())
            .enumerate()
        {
            let value = bytes.as_mut_ptr() as *mut u64;
            unsafe {
                *value = values[index];
            }
        }

        let _ = mmap.flush();

        self
    }
}

/// This is a wrapper type that is used to trigger refresh of userspace metrics
/// from the BPF maps.
///
/// The 'static lifetime bound is required for ouroboros self-referencing type.
pub struct Bpf<T: 'static> {
    bpf: _Bpf<T>,
}

impl<T: 'static + GetMap> Bpf<T> {
    pub fn refresh(&mut self, elapsed: Duration) {
        self.bpf.refresh(elapsed);
    }
}

/// This is an inner type that is self-referencing and owns both the actual BPF
/// program and the counter sets and histograms that reference maps in that
/// same BPF program.
#[self_referencing]
struct _Bpf<T: 'static> {
    skel: T,
    #[borrows(skel)]
    #[covariant]
    counters: Vec<BpfCounters<'this>>,
    #[borrows(skel)]
    #[covariant]
    histograms: Vec<BpfHistogram<'this>>,
}

impl<T: 'static + GetMap> _Bpf<T> {
    pub fn from_skel(skel: T) -> Self {
        _BpfBuilder {
            skel,
            counters_builder: |_| Vec::new(),
            histograms_builder: |_| Vec::new(),
        }
        .build()
    }

    pub fn map(&self, name: &str) -> &libbpf_rs::Map {
        self.with(|this| this.skel.map(name))
    }

    pub fn add_counters(mut self, name: &str, counters: Vec<CounterWithHist>) -> Self {
        self.with_mut(|this| {
            this.counters.push(BpfCounters::new(
                this.skel.map(name),
                counters,
                Default::default(),
            ));
        });
        self
    }

    pub fn add_counters_with_percpu(
        mut self,
        name: &str,
        counters: Vec<CounterWithHist>,
        percpu_counters: Arc<PercpuCounters>,
    ) -> Self {
        self.with_mut(|this| {
            this.counters.push(BpfCounters::new(
                this.skel.map(name),
                counters,
                percpu_counters,
            ));
        });
        self
    }

    pub fn add_histogram(mut self, name: &str, histogram: &'static RwLockHistogram) -> Self {
        self.with_mut(|this| {
            this.histograms
                .push(BpfHistogram::new(this.skel.map(name), histogram));
        });
        self
    }

    pub fn refresh(&mut self, elapsed: Duration) {
        self.with_mut(|this| {
            for c in this.counters.iter_mut() {
                c.refresh(Some(elapsed));
            }
            for h in this.histograms.iter_mut() {
                h.refresh();
            }
        })
    }
}
