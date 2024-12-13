/// Collects CPU usage stats using BPF and traces:
/// * `cpuacct_account_field`
///
/// And produces these stats:
/// * `cpu_usage/busy`
/// * `cpu_usage/user`
/// * `cpu_usage/nice`
/// * `cpu_usage/system`
/// * `cpu_usage/softirq`
/// * `cpu_usage/irq`
/// * `cpu_usage/steal`
/// * `cpu_usage/guest`
/// * `cpu_usage/guest_nice`

const NAME: &str = "cpu_usage";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_usage.bpf.rs"));
}

use bpf::*;

use crate::common::*;
use crate::samplers::cpu::linux::stats::*;
use crate::*;

use std::sync::Arc;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .cpu_counters("counters", vec![&CPU_BUSY, &CPU_USER, &CPU_NICE, &CPU_SYSTEM, &CPU_SOFTIRQ, &CPU_IRQ, &CPU_STEAL, &CPU_GUEST, &CPU_GUEST_NICE])
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "counters" => &self.maps.counters,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} cpuacct_account_field() BPF instruction count: {}",
            self.progs.cpuacct_account_field_kprobe.insn_cnt()
        );
    }
}
