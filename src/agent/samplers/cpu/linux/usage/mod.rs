//! Collects CPU usage stats using BPF and traces:
//! * `cpuacct_account_field`
//! * `softirq_entry`
//! * `softirq_exit`
//!
//! And produces these stats:
//! * `cpu_usage`
//! * `cgroup_cpu_usage`
//! * `softirq`
//! * `softirq_time`
//!
//! Note: softirq is included because we need to trace softirq entry/exit in
//! order to provide accurate accounting of cpu_usage for softirq. That makes
//! these additional metrics free.

const NAME: &str = "cpu_usage";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_usage.bpf.rs"));
}

use bpf::*;

use crate::agent::bpf::cgroup;
use crate::agent::*;

use std::sync::Arc;

mod stats;

use stats::*;

crate::impl_cgroup_info!(bpf::types::cgroup_info);

// Define all cgroup metrics in one place
static CGROUP_METRICS: &[&dyn cgroup::MetricGroup] = &[
    &CGROUP_CPU_USAGE_USER,
    &CGROUP_CPU_USAGE_SYSTEM,
];

fn handle_cgroup_event(data: &[u8]) -> i32 {
    let mut cgroup_info = bpf::types::cgroup_info::default();
    
    if plain::copy_from_bytes(&mut cgroup_info, data).is_ok() {
        let name = cgroup::format_cgroup_name(&cgroup_info);
        let id = cgroup::CgroupInfo::id(&cgroup_info) as usize;
        
        // Set metadata for all metrics
        for metric in CGROUP_METRICS {
            cgroup::set_name(id, &name, metric);
        }
    }
    
    0
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set root cgroup name for all metrics
    for metric in CGROUP_METRICS {
        cgroup::set_name(1, "/", metric);
    }

    let cpu_usage = vec![&CPU_USAGE_USER, &CPU_USAGE_SYSTEM];

    let softirq = vec![
        &SOFTIRQ_HI,
        &SOFTIRQ_TIMER,
        &SOFTIRQ_NET_TX,
        &SOFTIRQ_NET_RX,
        &SOFTIRQ_BLOCK,
        &SOFTIRQ_IRQ_POLL,
        &SOFTIRQ_TASKLET,
        &SOFTIRQ_SCHED,
        &SOFTIRQ_HRTIMER,
        &SOFTIRQ_RCU,
    ];

    let softirq_time = vec![
        &SOFTIRQ_TIME_HI,
        &SOFTIRQ_TIME_TIMER,
        &SOFTIRQ_TIME_NET_TX,
        &SOFTIRQ_TIME_NET_RX,
        &SOFTIRQ_TIME_BLOCK,
        &SOFTIRQ_TIME_IRQ_POLL,
        &SOFTIRQ_TIME_TASKLET,
        &SOFTIRQ_TIME_SCHED,
        &SOFTIRQ_TIME_HRTIMER,
        &SOFTIRQ_TIME_RCU,
    ];

    let bpf = BpfBuilder::new(
        NAME,
        BpfProgStats {
            run_time: &BPF_RUN_TIME,
            run_count: &BPF_RUN_COUNT,
        },
        ModSkelBuilder::default,
    )
    .cpu_counters("cpu_usage", cpu_usage)
    .cpu_counters("softirq", softirq)
    .cpu_counters("softirq_time", softirq_time)
    .packed_counters("cgroup_user", &CGROUP_CPU_USAGE_USER)
    .packed_counters("cgroup_system", &CGROUP_CPU_USAGE_SYSTEM)
    .ringbuf_handler("cgroup_info", handle_cgroup_event)
    .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_user" => &self.maps.cgroup_user,
            "cgroup_system" => &self.maps.cgroup_system,
            "cpu_usage" => &self.maps.cpu_usage,
            "softirq" => &self.maps.softirq,
            "softirq_time" => &self.maps.softirq_time,
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
