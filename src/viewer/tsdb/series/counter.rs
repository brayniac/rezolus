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

    pub fn average_rate(&self) -> Option<f64> {
        // Use iterative approach to handle multiple resets properly
        if self.inner.len() < 2 {
            return None;
        }
        
        let mut total_value_increase = 0u64;
        let mut total_time = 0u64;
        let mut prev: Option<(u64, u64)> = None;
        
        for (ts, value) in self.inner.iter() {
            if let Some((prev_ts, prev_v)) = prev {
                let delta = value.wrapping_sub(prev_v);
                let time_delta = ts.wrapping_sub(prev_ts);
                
                // Only include segments where delta < 2^63 (not a reset)
                if delta < (1 << 63) && time_delta > 0 {
                    total_value_increase = total_value_increase.saturating_add(delta);
                    total_time = total_time.saturating_add(time_delta);
                }
            }
            prev = Some((*ts, *value));
        }
        
        if total_time > 0 {
            Some(total_value_increase as f64 / total_time as f64)
        } else {
            None
        }
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