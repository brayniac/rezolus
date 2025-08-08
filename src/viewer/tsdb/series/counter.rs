use super::*;

/// Represents a series of counter readings.
#[derive(Default, Clone)]
pub struct CounterSeries {
    inner: BTreeMap<u64, u64>,
}

impl CounterSeries {
    pub fn insert(&mut self, timestamp: u64, value: u64) {
        self.inner.insert(timestamp, value);
    }

    pub fn rate(&self) -> UntypedSeries {
        let mut rates = UntypedSeries::default();
        let mut prev: Option<(u64, u64)> = None;

        for (ts, value) in self.inner.iter() {
            if let Some((prev_ts, prev_v)) = prev {
                let delta = value.wrapping_sub(prev_v);

                if delta < 1 << 63 {
                    let duration = ts.wrapping_sub(prev_ts);

                    let rate = delta as f64 / (duration as f64 / 1000000000.0);

                    rates.inner.insert(*ts, rate);
                }
            }

            prev = Some((*ts, *value));
        }

        rates
    }

    /// Convert counter values to untyped series without applying rate
    pub fn untyped(&self) -> UntypedSeries {
        let mut result = UntypedSeries::default();
        for (ts, value) in self.inner.iter() {
            result.inner.insert(*ts, *value as f64);
        }
        result
    }
}