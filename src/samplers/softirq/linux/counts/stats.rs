use metriken::*;

use crate::common::*;

/*
 * IRQs per-CPU
 */

#[metric(
    name = "softirq_interrupts",
    description = "The total number of interrupts",
    metadata = { unit = "interrupts", kind = "other" }
)]
pub static SOFTIRQ_INTERRUPTS_OTHER: CounterGroup = CounterGroup::new(MAX_CPUS);
