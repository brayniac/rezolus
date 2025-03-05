mod counts;
mod time;

use crate::common::MAX_IRQS;

pub fn irq_lut() -> Vec<u64> {
    (0..MAX_IRQS)
        .map(|_id| {
            0
        })
        .collect()
}
