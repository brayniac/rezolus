use super::*;

use core::sync::atomic::Ordering;

/// Represents a distribution in a BPF map. The distribution must be created
/// with:
///
/// ```c
/// struct {
///     __uint(type, BPF_MAP_TYPE_ARRAY);
///     __uint(map_flags, BPF_F_MMAPABLE);
///     __type(key, u32);
///     __type(value, u64);
///     __uint(max_entries, 7424);
/// } some_distribution_name SEC(".maps");
/// ```
///
/// This distribution must also be indexed into using the `value_to_index`
/// helper from `histogram.h`. This results in a histogram that uses 64bit
/// counters and covers the entire range of u64 values. This histogram occupies
/// 60KB in kernel space and an additional ~3.5MB in user space.
///
/// The distribution should be given some meaningful name in the BPF program.
pub struct Distribution<'a> {
    _map: &'a libbpf_rs::Map,
    mmap: memmap2::MmapMut,
    heatmap: &'static Histogram,
}

impl<'a> Distribution<'a> {
    pub fn new(map: &'a libbpf_rs::Map, heatmap: &'static Histogram) -> Self {
        let fd = map.as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(HISTOGRAM_PAGES * PAGE_SIZE)
                .map_mut(&file)
                .expect("failed to mmap() bpf distribution")
        };

        Self {
            _map: map,
            mmap,
            heatmap,
        }
    }

    pub fn refresh(&mut self, now: Instant) {
        let buckets = self.heatmap.as_slice();

        for (idx, bucket) in buckets.iter().enumerate() {
            let start = idx * std::mem::size_of::<u64>();

            if start + 7 >= self.mmap.len() {
                break;
            }

            let val = u64::from_ne_bytes([
                self.mmap[start + 0],
                self.mmap[start + 1],
                self.mmap[start + 2],
                self.mmap[start + 3],
                self.mmap[start + 4],
                self.mmap[start + 5],
                self.mmap[start + 6],
                self.mmap[start + 7],
            ]);

            bucket.store(val, Ordering::Relaxed);
        }

        self.heatmap.snapshot(now);
    }
}
