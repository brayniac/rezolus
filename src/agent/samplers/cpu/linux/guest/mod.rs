//! Counts what guests execute on the CPUs the PMU reservation covers:
//!
//! * `cpu_guest_cycles` - cycles spent in guest mode, per CPU
//! * `cpu_guest_instructions` - instructions retired in guest mode, per CPU
//! * `cpu_guest_pmc_index{event}` - the hardware counter each event occupies
//!   on that CPU, or -1 while it occupies none
//!
//! Both events are opened with perf's `exclude_host`, which on AMD sets the
//! event select's GuestOnly bit and on Intel is switched by KVM at VM entry
//! and exit. They count a guest's execution on that core, kernel and user,
//! and nothing the host runs there. That works on a stock kernel.
//!
//! The index is for a guest that reads these counters itself. With kvm-amd's
//! `rdpmc_passthrough` parameter (infra `host-setup/kvm-rdpmc`), a VM with no
//! vPMU executes RDPMC without a VM exit and reads the physical counters, at
//! ~8 ns a read instead of ~14 us. It has to be told which counter holds which
//! event, and that is perf's choice. The kernel publishes it in each event's
//! mmap page, but only for a process that has mapped the page and only from
//! the next time the event is scheduled in. So each event is mapped before
//! the group is enabled, and the index is re-read at every refresh.
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

use std::os::fd::AsRawFd;

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

/// One event's mmap page, read for the counter index the kernel publishes.
struct UserPage {
    ptr: *mut libc::c_void,
    len: usize,
}

// The page is only read, through volatile loads under the kernel's seqlock.
unsafe impl Send for UserPage {}

impl UserPage {
    fn map(counter: &Counter) -> Result<Self, std::io::Error> {
        let len = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        // One page: the metadata page alone, no ring buffer. PROT_READ is
        // enough to read it and is what sets the event's user-read flag.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ,
                libc::MAP_SHARED,
                counter.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { ptr, len })
    }

    /// The RDPMC index of the counter holding this event, if it holds one.
    fn index(&self) -> Option<u32> {
        use perf_event_open_sys::bindings::perf_event_mmap_page;
        use std::ptr::{addr_of, read_volatile};
        use std::sync::atomic::{fence, Ordering};

        let page = self.ptr as *const perf_event_mmap_page;
        loop {
            // SAFETY: `page` is a live read-only mapping of the event's
            // metadata page for as long as `self` exists.
            let (seq, caps, index) = unsafe {
                let seq = read_volatile(addr_of!((*page).lock));
                fence(Ordering::Acquire);
                let caps = read_volatile(addr_of!((*page).__bindgen_anon_1.capabilities));
                let index = read_volatile(addr_of!((*page).index));
                fence(Ordering::Acquire);
                (seq, caps, index)
            };
            if unsafe { read_volatile(addr_of!((*page).lock)) } != seq {
                continue;
            }
            // capabilities bit 2 is cap_user_rdpmc. `index` is 1-based; 0 means
            // the event is not on a counter right now.
            let cap_user_rdpmc = caps & (1 << 2) != 0;
            return (cap_user_rdpmc && index != 0).then(|| index - 1);
        }
    }
}

impl Drop for UserPage {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr, self.len);
        }
    }
}

struct Cpu {
    id: usize,
    cycles: Counter,
    instructions: Counter,
    cycles_page: UserPage,
    instructions_page: UserPage,
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

        // Mapped before enabling: the index is published from the next
        // schedule-in after the mapping, and enabling is that schedule-in.
        let cycles_page = UserPage::map(&cycles)?;
        let instructions_page = UserPage::map(&instructions)?;

        cycles.enable_group()?;

        Ok(Self {
            id,
            cycles,
            instructions,
            cycles_page,
            instructions_page,
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
        let index = |p: &UserPage| p.index().map(|i| i as i64).unwrap_or(-1);
        let _ = CPU_GUEST_PMC_INDEX_CYCLES.set(self.id, index(&self.cycles_page));
        let _ = CPU_GUEST_PMC_INDEX_INSTRUCTIONS.set(self.id, index(&self.instructions_page));
    }
}
