use metriken::*;

use crate::common::*;

/*
 * IRQs per-CPU
 */

#[metric(
    name = "irq_interrupts",
    description = "The total number of interrupts",
    metadata = { unit = "interrupts", kind = "other" }
)]
pub static IRQ_INTERRUPTS_OTHER: CounterGroup = CounterGroup::new(MAX_CPUS);
