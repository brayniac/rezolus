use core::time::Duration;
use metriken::AtomicHistogram;
use metriken::LazyCounter;

/// A `Counter` is a wrapper type that enables us to automatically calculate
/// percentiles for secondly rates between subsequent counter observations.
///
/// To do this, it contains the current reading, previous reading, and
/// optionally a histogram to store rate observations.
pub struct CounterWithHist {
    counter: &'static LazyCounter,
    histogram: &'static AtomicHistogram,
}

impl CounterWithHist {
    /// Construct a new counter that wraps a `metriken` counter and optionally a
    /// `metriken` histogram.
    pub fn new(counter: &'static LazyCounter, histogram: &'static AtomicHistogram) -> Self {
        Self { counter, histogram }
    }

    /// Updates the counter by setting it to a new value. If this counter has a
    /// histogram it also calculates a secondly rate since the last reading
    /// and increments the histogram.
    pub fn set(&mut self, elapsed: Option<Duration>, value: u64) -> u64 {
        if let Some(elapsed) = elapsed.map(|e| e.as_secs_f64()) {
            if let Some(previous) =
                metriken::Lazy::<metriken::Counter>::get(self.counter).map(|c| c.value())
            {
                let delta = value.wrapping_sub(previous);

                let _ = self.histogram.increment((delta as f64 / elapsed) as _);
            }
        }

        self.counter.set(value)
    }

    /// Updates the counter by incrementing it by some value. If this counter
    /// has a histogram, it normalizes the increment to a secondly rate and
    /// increments the histogram too.
    #[allow(dead_code)]
    pub fn add(&mut self, elapsed: Option<Duration>, delta: u64) -> u64 {
        if let Some(elapsed) = elapsed.map(|e| e.as_secs_f64()) {
            let _ = self.histogram.increment((delta as f64 / elapsed) as _);
        }

        self.counter.add(delta)
    }
}
