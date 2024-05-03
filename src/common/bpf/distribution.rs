use super::*;
use ringlog::*;

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
/// 60KB in kernel space and an additional 60KB in user space.
///
/// The distribution should be given some meaningful name in the BPF program.
pub struct Distribution<'a> {
    _map: &'a libbpf_rs::Map,
    mmap: memmap2::MmapMut,
    pages: usize,
    buffer: Vec<u64>,
    histogram: &'static RwLockHistogram,
}

impl<'a> Distribution<'a> {
    pub fn new(
        map: &'a libbpf_rs::Map,
        histogram: &'static RwLockHistogram,
    ) -> Result<Self, ()> {
        let buckets = histogram.config().total_buckets();
        let pages = buckets_to_pages(buckets);

        let fd = map.as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(pages * PAGE_SIZE)
                .map_mut(&file)
                .expect("failed to mmap() bpf distribution")
        };

        Ok(Self {
            _map: map,
            mmap,
            pages,
            buffer: Vec::new(),
            histogram,
        })
    }

    pub fn refresh(&mut self) {
        // If the mmap'd region is properly aligned we can more efficiently
        // update the histogram. Otherwise, fall-back to the old strategy.

        let (_prefix, buckets, _suffix) = unsafe { self.mmap.align_to::<u64>() };

        let expected_len = self.pages * PAGE_SIZE / 8;

        let histogram_buckets = self.histograms[0].config().total_buckets();

        if buckets.len() == expected_len {
            let mut offset = 0;

            for histogram in &self.histograms {
                let _ = histogram.update_from(&buckets[offset..(offset + histogram_buckets)]);
                offset += histogram_buckets;
            }
        } else {
            warn!("mmap region misaligned or did not have expected number of values {} != {expected_len}", buckets.len());
        
            self.buffer.resize(histogram_buckets, 0);

            for histogram in &self.histograms {
                for (idx, bucket) in self.buffer.iter_mut().enumerate() {
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

                    *bucket = val;
                }

                let _ = histogram
                    .update_from(&self.buffer[0..histogram_buckets]);
            }
        }
    }
}

pub struct MultiDistribution<'a> {
    _map: &'a libbpf_rs::Map,
    mmap: memmap2::MmapMut,
    config: histogram::Config,
    pages: usize,
    buffer: Vec<u64>,
    histograms: Box<[Option<Arc<RwLockHistogram>>]>,
}

impl<'a> MultiDistribution<'a> {
    pub fn new(
        map: &'a libbpf_rs::Map,
        config: histogram::Config,
        len: usize,
    ) -> Result<Self, ()> {
        let buckets = config.total_buckets();
        let pages = buckets_to_pages(buckets * len);

        let fd = map.as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(pages * PAGE_SIZE)
                .map_mut(&file)
                .expect("failed to mmap() bpf distribution")
        };

        let histograms = vec![None; len].into_boxed_slice();

        Ok(Self {
            _map: map,
            mmap,
            pages,
            buffer: Vec::new(),
            histograms,
        })
    }

    pub fn register(&mut self, index: usize, histogram: Arc<RwLockHistogram>) -> Result<(), ()> {
        if index >= self.histograms.len() {
            error!("index out of range");
            return Err(());
        }

        if self.histograms[index].is_some() {
            error!("an existing histogram is registered at this index");
            return Err(());
        }

        if histogram.config() != self.config {
            error!("histogram config does not match");
            return Err(());
        }

        self.histograms[index] = Some(histogram);
    }

    pub fn deregister(&mut self, index: usize) -> Result<(), ()> {
        if index >= self.histograms.len() {
            error!("index out of range");
            return Err(());
        }

        if self.histograms[index].is_none() {
            error!("no histogram is registered at this index");
            return Err(());
        }

        self.histograms[index] = None;
    }

    pub fn refresh(&mut self) {
        // If the mmap'd region is properly aligned we can more efficiently
        // update the histogram. Otherwise, fall-back to the old strategy.

        let (_prefix, buckets, _suffix) = unsafe { self.mmap.align_to::<u64>() };

        let expected_len = self.pages * PAGE_SIZE / 8;

        let histogram_buckets = self.config.total_buckets();
        let mut offset = 0;

        if buckets.len() == expected_len {
            for histogram in &self.histograms {
                if let Some(histogram) = histogram {
                    let _ = histogram.update_from(&buckets[offset..(offset + histogram_buckets)]);
                }
                
                offset += histogram_buckets;
            }
        } else {
            warn!("mmap region misaligned or did not have expected number of values {} != {expected_len}", buckets.len());
        
            self.buffer.resize(histogram_buckets, 0);

            for histogram in &self.histograms {
                if let Some(histogram) = histogram {
                    for (idx, bucket) in self.buffer.iter_mut().enumerate() {
                        let start = (idx + offset) * std::mem::size_of::<u64>();

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

                        *bucket = val;
                    }

                    let _ = histogram
                        .update_from(&self.buffer[0..histogram_buckets]);
                }
                
                offset += histogram_buckets;
            }
        }
    }
}
