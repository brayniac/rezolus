use metriken::*;

use crate::common::*;

/*
 * IRQs per-CPU
 */

#[metric(
    name = "irq_time",
    description = "The time spent in irq handlers",
    metadata = { unit = "nanoseconds", kind = "other" }
)]
pub static IRQ_TIME_OTHER: CounterGroup = CounterGroup::new(MAX_CPUS);
