/// Collects Syscall stats using BPF and traces:
/// * `raw_syscalls/sys_enter`
///
/// And produces these stats:
/// * `syscall/total`
/// * `syscall/read`
/// * `syscall/write`
/// * `syscall/poll`
/// * `syscall/lock`
/// * `syscall/time`
/// * `syscall/sleep`
/// * `syscall/socket`
/// * `syscall/yield`

const NAME: &str = "cpu_perf_cgroups";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_perf.bpf.rs"));
}

use bpf::*;

use crate::common::*;
use crate::samplers::cpu::linux::perf::*;
// use crate::samplers::syscall::linux::stats::*;
// use crate::samplers::syscall::linux::syscall_lut;
use crate::*;

use std::sync::Arc;
use std::os::fd::RawFd;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // let counters = vec![
    //     &SYSCALL_TOTAL,
    //     &SYSCALL_READ,
    //     &SYSCALL_WRITE,
    //     &SYSCALL_POLL,
    //     &SYSCALL_LOCK,
    //     &SYSCALL_TIME,
    //     &SYSCALL_SLEEP,
    //     &SYSCALL_SOCKET,
    //     &SYSCALL_YIELD,
    // ];

    let fds = {
        let perf_events = PERF_EVENTS.blocking_lock();

        perf_events.file_descriptors()
    };

    let cpus = common::linux::cpus()?;

    let cycles: Vec<Option<RawFd>> = cpus.iter().map(|cpu| fds.get(*cpu, Counter::Cycles)).collect();
    let instructions: Vec<Option<RawFd>> = cpus.iter().map(|cpu| fds.get(*cpu, Counter::Instructions)).collect();

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        // .counters("counters", counters)
        // .map("syscall_lut", syscall_lut())
        .perf_events("cycles", cycles)
        .perf_events("instructions", instructions)
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cycles" => &self.maps.cycles,
            "instructions" => &self.maps.instructions,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} handle__sched_switch() BPF instruction count: {}",
            self.progs.handle__sched_switch.insn_cnt()
        );
    }
}
