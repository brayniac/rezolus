//! The reader for per-device block I/O maps (`bpf/blockdev.h`): counter maps
//! of one bank per (CPU, slot), and histogram maps of one slice per (CPU,
//! slot). One reader owns the `dev_t → slot` lookup, the
//! `blk_unknown` reports and every map indexed by the lookup, so a slot that
//! changes hands is zeroed in every map before the lookup sends a new disk's
//! requests to it. Which disk gets which slot is `blockdevs::SlotTable`'s
//! decision.
//!
//! # Windowing
//!
//! Each acquisition group is read in one sweep, bracketed by one acquisition,
//! as `Counters` and `HistogramBatch` do. The population is a bound, one past
//! the highest occupied slot, revised when the assignment changes; a vacant
//! slot under it reads absent.

use std::collections::HashMap;
use std::os::fd::AsFd;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use libbpf_rs::{Map, MapCore, MapFlags};
use metriken::group::SlotIdentity;
use metriken::{CounterGroup, RwLockCounterHistogramGroup};

use super::blockdevs::{self, Action, SlotTable};
use super::counters::CounterMap;
use super::{possible_cpus, whole_pages, PAGE_SIZE};
use crate::agent::timing::AcquisitionGroup;
use crate::agent::MAX_BLOCK_DEVICES;
use crate::{debug, info, warn};

/// Entries the lookup map holds. Must match `BLK_LOOKUP_ENTRIES` in
/// `blockdev.h`; the BPF side declares the map, so only the layout test reads
/// this.
#[cfg(test)]
const LOOKUP_ENTRIES: usize = 1024;

/// Entries of `blk_unknown`. Must match `BLK_UNKNOWN_ENTRIES` in `blockdev.h`.
pub(crate) const UNKNOWN_ENTRIES: usize = 64;

/// Bytes of a disk name in a `blk_unknown` entry (`BLK_NAME_LEN`).
const NAME_LEN: usize = 32;

/// Bytes of one `struct blk_unknown`: `dev`, `check`, `name`.
const UNKNOWN_SIZE: usize = 8 + NAME_LEN;

/// FNV-1a over `dev`'s four bytes, then `name`: the `check` a `blk_unknown`
/// entry carries (`blk_check` in `blockdev.h`).
pub(crate) fn unknown_check(dev: u32, name: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for b in dev.to_le_bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    for &b in name.iter().take(NAME_LEN) {
        if b == 0 {
            break;
        }
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}

/// The index of counter `idx` in CPU `cpu`'s bank for `slot`; the BPF side
/// computes the same (`blk_counter_idx` in `bpf/blockdev.h`).
pub(crate) fn bank_index(cpu: usize, slot: usize, idx: usize, bank_width: usize) -> usize {
    (cpu * MAX_BLOCK_DEVICES + slot) * bank_width + idx
}

/// The index of `bucket` in CPU `cpu`'s slice of `slot`'s histogram; the BPF
/// side computes the same (`blk_hist_idx` in `bpf/blockdev.h`).
pub(crate) fn slice_index(cpu: usize, slot: usize, bucket: usize, buckets: usize) -> usize {
    (cpu * MAX_BLOCK_DEVICES + slot) * buckets + bucket
}

/// A counter map with a slot dimension, and the `CounterGroup`s it fills, one
/// per counter in the map's order.
struct SlotCounters<'a> {
    map: CounterMap<'a>,
    counters: Vec<&'static CounterGroup>,
    group: &'static AcquisitionGroup,
}

/// A histogram map of one slice of `BLK_HIST_BUCKETS` per (possible CPU,
/// slot). The group it fills has the full layout (`published` buckets), whose
/// buckets past the slice stay zero.
struct SlotHistogram<'a> {
    _map: &'a Map<'a>,
    mmap: memmap2::MmapMut,
    histogram: &'static RwLockCounterHistogramGroup,
    buckets: usize,
    published: usize,
}

impl<'a> SlotHistogram<'a> {
    fn new(map: &'a Map, histogram: &'static RwLockCounterHistogramGroup) -> Self {
        let buckets = crate::agent::BLK_HIST_BUCKETS;
        let published = histogram.config().total_buckets();
        let len = whole_pages::<u64>(possible_cpus() * MAX_BLOCK_DEVICES * buckets) * PAGE_SIZE;
        let fd = map.as_fd();
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(len)
                .map_mut(&fd)
                .expect("failed to mmap() a per-device histogram map")
        };
        Self {
            _map: map,
            mmap,
            histogram,
            buckets,
            published,
        }
    }

    fn values(&self) -> &[u64] {
        let (_prefix, values, _suffix) = unsafe { self.mmap.align_to::<u64>() };
        values
    }

    fn values_mut(&mut self) -> &mut [u64] {
        let (_prefix, values, _suffix) = unsafe { self.mmap.align_to_mut::<u64>() };
        values
    }
}

pub struct BlockDevices<'a> {
    lookup: &'a Map<'a>,
    /// `blk_generation`: bumped after each change to the lookup, so the BPF
    /// side's per-CPU cache of the last mapping is not used past it.
    generation_map: &'a Map<'a>,
    generation: u64,
    _unknown_map: &'a Map<'a>,
    unknown: memmap2::MmapMut,
    identity: &'static SlotIdentity,
    counters: Vec<SlotCounters<'a>>,
    /// Histograms by acquisition group, in registration order.
    histograms: Vec<(&'static AcquisitionGroup, Vec<SlotHistogram<'a>>)>,
    table: SlotTable,
    /// The (dev, check) each `blk_unknown` entry held when last read, so an
    /// entry is decoded once per write.
    read_unknown: Vec<(u32, u32)>,
    /// Lookup entries whose insert failed, retried on each refresh.
    pending: HashMap<u32, usize>,
    last_scan: Instant,
    /// Disks beyond the cap at the last warning, so it fires on change.
    overflow: usize,
    /// Each slot's total over every map at the last refresh, and when it last
    /// changed: how long the slot's disk has been idle.
    activity: Vec<(u64, Instant)>,
}

/// One histogram registration: map, group the map fills, acquisition group.
pub type HistogramRegistration<'a> = (
    &'a Map<'a>,
    &'static RwLockCounterHistogramGroup,
    &'static AcquisitionGroup,
);

/// One counter registration: map, groups in the map's counter order,
/// acquisition group.
pub type CounterRegistration<'a> = (
    &'a Map<'a>,
    Vec<&'static CounterGroup>,
    &'static AcquisitionGroup,
);

impl<'a> BlockDevices<'a> {
    pub fn new(
        lookup: &'a Map,
        generation_map: &'a Map,
        unknown_map: &'a Map,
        identity: &'static SlotIdentity,
        counters: Vec<CounterRegistration<'a>>,
        histograms: Vec<HistogramRegistration<'a>>,
    ) -> Self {
        let banks = possible_cpus() * MAX_BLOCK_DEVICES;
        let counters = counters
            .into_iter()
            .map(|(map, counters, group)| SlotCounters {
                map: CounterMap::with_banks(map, counters.len(), banks)
                    .expect("failed to initialize a per-device counter map"),
                counters,
                group,
            })
            .collect();

        let mut batches: Vec<(&'static AcquisitionGroup, Vec<SlotHistogram<'a>>)> = Vec::new();
        for (map, histogram, group) in histograms {
            let h = SlotHistogram::new(map, histogram);
            match batches.iter_mut().find(|(g, _)| std::ptr::eq(*g, group)) {
                Some((_, list)) => list.push(h),
                None => batches.push((group, vec![h])),
            }
        }

        let len = whole_pages::<u8>(UNKNOWN_ENTRIES * UNKNOWN_SIZE) * PAGE_SIZE;
        let fd = unknown_map.as_fd();
        let unknown = unsafe {
            memmap2::MmapOptions::new()
                .len(len)
                .map_mut(&fd)
                .expect("failed to mmap() blk_unknown")
        };

        let this = Self {
            lookup,
            generation_map,
            generation: 0,
            _unknown_map: unknown_map,
            unknown,
            identity,
            counters,
            histograms: batches,
            table: SlotTable::default(),
            read_unknown: vec![(0, 0); UNKNOWN_ENTRIES],
            pending: HashMap::new(),
            last_scan: Instant::now(),
            overflow: 0,
            activity: vec![(0, Instant::now()); MAX_BLOCK_DEVICES],
        };

        for group in this.groups() {
            group.set_member_bound(1);
        }
        this.identity.assign(0, blockdevs::labels(blockdevs::OTHER));
        this
    }

    fn groups(&self) -> Vec<&'static AcquisitionGroup> {
        self.counters
            .iter()
            .map(|c| c.group)
            .chain(self.histograms.iter().map(|(g, _)| *g))
            .collect()
    }

    /// `blk_unknown` entry `idx`'s `dev` and `check`, as atomics: BPF writes
    /// them, and the reader clears `dev` to have a disk reported again.
    fn unknown_words(&self, idx: usize) -> (&AtomicU32, &AtomicU32) {
        let base = idx * UNKNOWN_SIZE;
        // SAFETY: the mapping is at least UNKNOWN_ENTRIES * UNKNOWN_SIZE bytes
        // and page aligned, and each entry starts on a multiple of 8 bytes.
        unsafe {
            let ptr = self.unknown.as_ptr().add(base) as *const AtomicU32;
            (&*ptr, &*ptr.add(1))
        }
    }

    /// Decode the reports BPF has written since the last refresh and give the
    /// disks slots.
    fn drain_unknown(&mut self) {
        for idx in 0..UNKNOWN_ENTRIES {
            let (dev_word, check_word) = self.unknown_words(idx);
            let dev = dev_word.load(Ordering::Acquire);
            let check = check_word.load(Ordering::Acquire);
            if dev == 0 || self.read_unknown[idx] == (dev, check) {
                continue;
            }
            let base = idx * UNKNOWN_SIZE + 8;
            let name_bytes: Vec<u8> = self.unknown[base..base + NAME_LEN].to_vec();
            if unknown_check(dev, &name_bytes) != check {
                // Two CPUs wrote the entry at once. Clear it so the next
                // request on either disk reports again.
                dev_word.store(0, Ordering::Release);
                self.read_unknown[idx] = (0, 0);
                continue;
            }
            self.read_unknown[idx] = (dev, check);
            let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
            let Ok(name) = std::str::from_utf8(&name_bytes[..end]) else {
                continue;
            };
            if name.is_empty() || self.table.knows(dev, name) {
                continue;
            }
            let evict = self.idle_slot();
            let actions = self.table.observe_or_evict(dev, name, || evict);
            self.apply(actions);
        }
    }

    /// Carry out the slot table's decisions, in order, then declare the new
    /// member bound. A slot is released only after every `dev_t` mapped to it
    /// is removed from the lookup, and assigned only after it is zeroed, so
    /// the next occupant starts from zero and no exposition pairs its labels
    /// with the previous disk's values.
    fn apply(&mut self, actions: Vec<Action>) {
        if actions.is_empty() {
            return;
        }
        for action in actions {
            match action {
                Action::Unmap { dev } => {
                    if let Err(e) = self.lookup.delete(&dev.to_ne_bytes()) {
                        debug!("block device slots: delete {dev:#x} from lookup: {e}");
                    }
                    // Before the slot is zeroed: a CPU's cached mapping of
                    // this dev_t must not outlive the lookup entry.
                    self.bump_generation();
                    self.pending.remove(&dev);
                    // Let BPF report the dev_t again if it reappears.
                    for idx in 0..UNKNOWN_ENTRIES {
                        let (dev_word, _) = self.unknown_words(idx);
                        if dev_word.load(Ordering::Acquire) == dev {
                            dev_word.store(0, Ordering::Release);
                            self.read_unknown[idx] = (0, 0);
                        }
                    }
                }
                Action::Release { slot } => {
                    for (_, list) in &self.histograms {
                        for h in list {
                            h.histogram.clear(slot);
                        }
                    }
                    for c in &self.counters {
                        for counter in &c.counters {
                            // u64::MAX is CounterGroup's never-written sentinel.
                            counter.set(slot, u64::MAX);
                        }
                    }
                    self.identity.release(slot);
                    self.zero(slot);
                    self.activity[slot] = (0, Instant::now());
                }
                Action::Assign { slot, label } => {
                    self.zero(slot);
                    self.activity[slot] = (0, Instant::now());
                    info!("block device slots: {label} has slot {slot}");
                    self.identity.assign(slot, blockdevs::labels(&label));
                }
                Action::Map { dev, slot } => self.map(dev, slot),
            }
        }
        // Stored before the next `finish()`, as the bound contract requires.
        let bound = self.table.bound();
        for group in self.groups() {
            group.set_member_bound(bound);
        }
    }

    /// Invalidate every CPU's cached mapping (`blk_cache` in `blockdev.h`).
    fn bump_generation(&mut self) {
        self.generation += 1;
        if let Err(e) = self.generation_map.update(
            &0u32.to_ne_bytes(),
            &self.generation.to_ne_bytes(),
            MapFlags::ANY,
        ) {
            warn!("block device slots: could not update blk_generation: {e}");
        }
    }

    /// The occupied slot whose sums have been unchanged longest, if that is at
    /// least `EVICT_IDLE`: the slot a full table gives to a new disk.
    fn idle_slot(&self) -> Option<usize> {
        self.table
            .members()
            .into_iter()
            .filter(|&slot| slot != 0)
            .map(|slot| (slot, self.activity[slot].1))
            .filter(|(_, since)| since.elapsed() >= blockdevs::EVICT_IDLE)
            .min_by_key(|(_, since)| *since)
            .map(|(slot, _)| slot)
    }

    /// Map `dev` to `slot` in the lookup, or keep it for the next refresh to
    /// retry. Until it is mapped, the disk's requests count in slot 0.
    fn map(&mut self, dev: u32, slot: usize) {
        match self.lookup.update(
            &dev.to_ne_bytes(),
            &(slot as u32).to_ne_bytes(),
            MapFlags::ANY,
        ) {
            Ok(()) => {
                self.pending.remove(&dev);
                self.bump_generation();
            }
            Err(e) => {
                if self.pending.insert(dev, slot).is_none() {
                    warn!("block device slots: could not map {dev:#x} to slot {slot}: {e}");
                }
            }
        }
    }

    /// Zero every map's values for `slot`. Only called while no disk maps to
    /// the slot, so no BPF increment races the stores beyond one already in
    /// flight when the lookup entry was removed.
    fn zero(&mut self, slot: usize) {
        let cpus = possible_cpus();
        for c in &mut self.counters {
            let bank_width = c.map.bank_width();
            let width = c.counters.len();
            let values = c.map.values_mut();
            for cpu in 0..cpus {
                let base = bank_index(cpu, slot, 0, bank_width);
                values[base..base + width].fill(0);
            }
        }
        for (_, list) in &mut self.histograms {
            for h in list {
                let buckets = h.buckets;
                let values = h.values_mut();
                for cpu in 0..cpus {
                    let base = slice_index(cpu, slot, 0, buckets);
                    values[base..base + buckets].fill(0);
                }
            }
        }
    }

    /// Give newly reported disks their slots, free the slots of disks that are
    /// gone (at most every `RESCAN_INTERVAL`), retry failed lookup inserts,
    /// then sum each occupied slot and publish.
    pub fn refresh(&mut self) {
        self.drain_unknown();

        if self.last_scan.elapsed() >= blockdevs::RESCAN_INTERVAL {
            self.last_scan = Instant::now();
            match blockdevs::scan(std::path::Path::new(blockdevs::SYS_BLOCK)) {
                Ok(present) => {
                    let actions = self.table.retain(&present);
                    self.apply(actions);
                    // Disks in slot 0 while a slot is idle: free the slot.
                    if let Some(slot) = self.idle_slot().filter(|_| self.table.overflow() > 0) {
                        info!(
                            "block device slots: slot {slot} is idle; freed for the disks in {}",
                            blockdevs::OTHER
                        );
                        let actions = self.table.reclaim(slot);
                        self.apply(actions);
                    }
                }
                Err(e) => warn!(
                    "block device slots: could not read {}: {e}",
                    blockdevs::SYS_BLOCK
                ),
            }
        }

        let overflow = self.table.overflow();
        if overflow > self.overflow {
            warn!(
                "block device slots: {overflow} disk(s) beyond the {} slots are counted as \
                 device=\"{}\"",
                MAX_BLOCK_DEVICES - 1,
                blockdevs::OTHER
            );
        }
        self.overflow = overflow;

        let pending: Vec<(u32, usize)> = self.pending.iter().map(|(d, s)| (*d, *s)).collect();
        for (dev, slot) in pending {
            self.map(dev, slot);
        }

        let members = self.table.members();
        let cpus = possible_cpus();
        let mut totals = vec![0u64; MAX_BLOCK_DEVICES];

        for c in &self.counters {
            let bank_width = c.map.bank_width();
            let width = c.counters.len();
            let mut sums = vec![0u64; width];
            let acq = c.group.acquire();
            for &slot in &members {
                sums.fill(0);
                let values = c.map.values();
                for cpu in 0..cpus {
                    let base = bank_index(cpu, slot, 0, bank_width);
                    for (sum, value) in sums.iter_mut().zip(&values[base..base + width]) {
                        *sum = sum.wrapping_add(*value);
                    }
                }
                for (counter, sum) in c.counters.iter().zip(&sums) {
                    counter.set(slot, *sum);
                }
                totals[slot] = sums.iter().fold(totals[slot], |a, v| a.wrapping_add(*v));
            }
            acq.finish();
        }

        for (group, list) in &self.histograms {
            let acq = group.acquire();
            for h in list {
                // The published layout; buckets past the slice stay zero.
                let mut sum = vec![0u64; h.published];
                let values = h.values();
                for &slot in &members {
                    sum.fill(0);
                    for cpu in 0..cpus {
                        let base = slice_index(cpu, slot, 0, h.buckets);
                        for (s, v) in sum[..h.buckets]
                            .iter_mut()
                            .zip(&values[base..base + h.buckets])
                        {
                            *s = s.wrapping_add(*v);
                        }
                    }
                    let _ = h.histogram.update_from(slot, &sum);
                    totals[slot] = sum.iter().fold(totals[slot], |a, v| a.wrapping_add(*v));
                }
            }
            acq.finish();
        }

        let now = Instant::now();
        for &slot in &members {
            if self.activity[slot].0 != totals[slot] {
                self.activity[slot] = (totals[slot], now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The check the reader verifies is FNV-1a over the dev_t's bytes then
    /// the name, as `blk_check` computes it.
    #[test]
    fn unknown_check_is_fnv1a_of_dev_then_name() {
        let mut name = [0u8; NAME_LEN];
        name[..7].copy_from_slice(b"nvme0n1");
        let mut h: u32 = 2166136261;
        for b in (259u32 << 20).to_le_bytes().iter().chain(b"nvme0n1") {
            h = (h ^ *b as u32).wrapping_mul(16777619);
        }
        assert_eq!(unknown_check(259 << 20, &name), h);
        // A different name or dev_t gives a different check.
        assert_ne!(unknown_check(259 << 20, b"nvme1n1"), h);
        assert_ne!(unknown_check(8 << 20, &name), h);
    }

    /// Two CPUs never share a bank, and slot banks are contiguous per CPU.
    #[test]
    fn bank_index_separates_cpus_and_slots() {
        let width = 8;
        assert_eq!(bank_index(0, 0, 0, width), 0);
        assert_eq!(bank_index(0, 1, 0, width), width);
        assert_eq!(bank_index(1, 0, 0, width), MAX_BLOCK_DEVICES * width);
        assert_eq!(
            bank_index(1, 2, 3, width),
            (MAX_BLOCK_DEVICES + 2) * width + 3
        );
    }

    #[test]
    fn slice_index_separates_cpus_and_slots() {
        let buckets = crate::agent::BLK_HIST_BUCKETS;
        assert_eq!(slice_index(0, 1, 0, buckets), buckets);
        assert_eq!(
            slice_index(1, 0, 5, buckets),
            MAX_BLOCK_DEVICES * buckets + 5
        );
        // 272 buckets are whole cache lines, so every slice starts on a line.
        assert_eq!(buckets * 8 % 64, 0);
    }

    /// `blockdev.h` declares the same slot count and computes the same
    /// indexes.
    #[test]
    fn bpf_header_has_the_same_layout() {
        let header = include_str!("blockdev.h");
        assert!(header.contains(&format!("#define MAX_BLOCK_DEVICES {MAX_BLOCK_DEVICES}\n")));
        assert!(header.contains(&format!(
            "#define BLK_HIST_BUCKETS {}\n",
            crate::agent::BLK_HIST_BUCKETS
        )));
        // The grouping-power-3 buckets below 2^36: a prefix of the published
        // 496-bucket layout.
        assert_eq!(crate::agent::BLK_HIST_BUCKETS, (36 - 3 + 1) << 3);
        let full = histogram::Config::new(3, 64).unwrap();
        let bounded = histogram::Config::new(3, 36).unwrap();
        assert_eq!(bounded.total_buckets(), crate::agent::BLK_HIST_BUCKETS);
        assert!(full.total_buckets() > crate::agent::BLK_HIST_BUCKETS);
        let bucket = |config: &histogram::Config, value: u64| {
            let mut h = histogram::Histogram::with_config(config);
            h.increment(value).unwrap();
            h.as_slice().iter().position(|&c| c == 1).unwrap()
        };
        for value in [0u64, 1, 7, 8, 1000, 1 << 20, (1 << 36) - 1] {
            assert_eq!(bucket(&full, value), bucket(&bounded, value), "{value}");
        }
        assert!(header.contains("#define BLK_HIST_POWER 3\n"));
        assert!(header.contains(
            "return (bpf_get_smp_processor_id() * MAX_BLOCK_DEVICES + slot) * width + counter;"
        ));
        assert!(header.contains(
            "return (bpf_get_smp_processor_id() * MAX_BLOCK_DEVICES + slot) * BLK_HIST_BUCKETS + bucket;"
        ));
        assert!(header.contains("u32 dev = (major << 20) | minor;"));
        assert!(header.contains(&format!("#define BLK_LOOKUP_ENTRIES {LOOKUP_ENTRIES}\n")));
        assert!(header.contains(&format!("#define BLK_UNKNOWN_ENTRIES {UNKNOWN_ENTRIES}\n")));
        assert_eq!(1 << 6, UNKNOWN_ENTRIES);
        assert!(header.contains("#define BLK_UNKNOWN_SHIFT 6"));
        assert!(header.contains(&format!("#define BLK_NAME_LEN {NAME_LEN}\n")));
    }
}
