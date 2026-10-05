//! Counts what guests execute on the CPUs the PMU reservation covers:
//!
//! * `cpu_guest_cycles` - cycles spent in guest mode, per CPU
//! * `cpu_guest_instructions` - instructions retired in guest mode, per CPU
//!
//! Both events are opened with perf's `exclude_host`, which on AMD sets the
//! event select's GuestOnly bit and on Intel is switched by KVM at VM entry
//! and exit. They count a guest's execution on that core, kernel and user,
//! and nothing the host runs there. That works on a stock kernel.
//!
//! The counters come out of `reserved_pmu_counters`, on the CPUs in
//! `reserved_pmu_cpus`: that is where guests run and what the reservation
//! keeps the other PMU samplers off. With no reservation the sampler is
//! refused. On a host where guests still have a vPMU, the two counters this
//! takes per CPU are two fewer for the guests.
//!
//! Opt-in: off unless `[samplers.cpu_guest] enabled = true`.

const NAME: &str = "cpu_guest";

use crate::agent::*;

use perf_event::events::Hardware;
use perf_event::{Builder, Counter, ReadFormat};
use tokio::sync::Mutex;

mod stats;

use stats::*;

fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let inner = GuestInner::new()?;

    Ok(Some(Box::new(Guest {
        inner: inner.into(),
    })))
}

#[distributed_slice(SAMPLERS)]
static SAMPLER_ENTRY: crate::agent::samplers::SamplerEntry = crate::agent::samplers::SamplerEntry {
    name: NAME,
    module: module_path!(),
    init,
};

struct Guest {
    inner: Mutex<GuestInner>,
}

#[async_trait]
impl Sampler for Guest {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn refresh(&self) {
        self.inner.lock().await.refresh();
    }
}

struct GuestInner {
    cpus: Vec<Cpu>,
}

impl GuestInner {
    fn new() -> Result<Self, std::io::Error> {
        // The reserved CPUs: the plan grants this sampler exactly those.
        let allowed = crate::agent::pmu::allowed_cpus(NAME, crate::agent::bpf::present_cpus());
        CPU_GUEST_ACQ.set_member_set(&allowed);

        let mut cpus = Vec::with_capacity(allowed.len());
        for id in allowed {
            match Cpu::new(id) {
                Ok(cpu) => cpus.push(cpu),
                Err(e) => debug!("{NAME}: CPU{id}: {e}"),
            }
        }

        if cpus.is_empty() {
            return Err(std::io::Error::other(
                "no CPUs available for guest-only counters",
            ));
        }

        Ok(Self { cpus })
    }

    fn refresh(&mut self) {
        let guard = CPU_GUEST_ACQ.acquire();
        for cpu in self.cpus.iter_mut() {
            cpu.refresh();
        }
        guard.finish();
    }
}

struct Cpu {
    id: usize,
    cycles: Counter,
    instructions: Counter,
}

impl Cpu {
    fn new(id: usize) -> Result<Self, std::io::Error> {
        // A group, so both events are on the PMU together or not at all: an
        // IPC from two counters scheduled at different times is not one.
        let mut cycles = Builder::new(Hardware::CPU_CYCLES)
            .one_cpu(id)
            .any_pid()
            .exclude_host(true)
            .exclude_guest(false)
            .exclude_kernel(false)
            .exclude_hv(false)
            .pinned(true)
            .read_format(
                ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING | ReadFormat::GROUP,
            )
            .build()?;
        let instructions = Builder::new(Hardware::INSTRUCTIONS)
            .one_cpu(id)
            .any_pid()
            .exclude_host(true)
            .exclude_guest(false)
            .exclude_kernel(false)
            .exclude_hv(false)
            .build_with_group(&mut cycles)?;

        cycles.enable_group()?;

        Ok(Self {
            id,
            cycles,
            instructions,
        })
    }

    fn refresh(&mut self) {
        if let Ok(group) = self.cycles.read_group() {
            if let Some(v) = group.get(&self.cycles) {
                let _ = CPU_GUEST_CYCLES.set(self.id, v.value());
            }
            if let Some(v) = group.get(&self.instructions) {
                let _ = CPU_GUEST_INSTRUCTIONS.set(self.id, v.value());
            }
        }
    }
}
