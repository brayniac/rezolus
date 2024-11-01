use crate::common;
use crate::*;

use tokio::sync::Mutex as AsyncMutex;

use parking_lot::Mutex as Mutex;

use std::sync::LazyLock;

mod counter;
mod group;

pub use counter::Counter;
pub use group::Reading;

use group::PerfGroup;

pub struct PerfEvents {
    thread: std::thread::JoinHandle<Result<(), libbpf_rs::Error>>,
    sync: SyncPrimitive,
    fds: Arc<PerfEventFds>,
}

pub struct PerfEventFds {
    inner: Vec<Option<PerfGroupFds>>,
}

impl PerfEventFds {
    pub fn cpu(cpu: usize) -> Option<PerfGroupFds> {
        self.get(cpu)
    }
}

impl PerfEvents {
    pub fn new() -> Self {
        let sync = SyncPrimitive::new();
        let sync2 = sync.clone();

        let initialized = Arc::new(AtomicBool::new(false));
        let initialized2 = initialized.clone();

        let groups = PerfGroups::new();

        let fds = groups.get_fds();

        let thread = std::thread::spawn(move || {
            // the sampling loop
            loop {
                // blocking wait until we are notified to start, no cpu consumed
                sync.wait_trigger();

                readings.lock();

                // refresh all the metrics

                for v in &mut counters {
                    v.refresh();
                }

                for v in &mut histograms {
                    v.refresh();
                }

                for v in &mut cpu_counters {
                    v.refresh();
                }

                // notify that we have finished running
                sync.notify();
            }
        });

        // wait for the sampler thread to either error out or finish initializing
        loop {
            if thread.is_finished() {
                if let Err(e) = thread.join().unwrap() {
                    return Err(e);
                } else {
                    // the thread can't terminate without an error
                    unreachable!();
                }
            }

            if initialized2.load(Ordering::Relaxed) {
                break;
            }
        }

        Ok(Self {
            thread,
            sync: sync2,
        })
    }
}



pub struct P

pub static PERF_GROUPS: LazyLock<Mutex<PerfGroups>> =
    LazyLock::new(|| Mutex::new(PerfGroups::new()));

/// Contains one `PerfGroup` per CPU.
pub struct PerfGroups {
    groups: Vec<Option<PerfGroup>>,
}

impl PerfGroups {
    /// Create a new `PerfGroup`
    pub fn new() -> Self {
        let cpus = common::linux::cpus().expect("failed to get inventory of CPUs");

        let mut groups = Vec::with_capacity(cpus.len());

        let mut initialized = 0;

        for cpu in &cpus {
            match PerfGroup::new(*cpu) {
                Ok(g) => {
                    groups.push(Some(g));
                    initialized += 1;
                }
                Err(_) => {
                    warn!("Failed to create the perf group on CPU {}", cpu);
                    groups.push(None);
                }
            };
        }

        info!(
            "PerfGroups created for {} out of {} cpus",
            initialized,
            cpus.len()
        );

        Self { groups }
    }

    /// Collect readings from all of the groups.
    pub fn readings(&mut self) -> Vec<Reading> {
        let mut result = Vec::new();

        for group in &mut self.groups {
            if let Ok(reading) = group.get_metrics() {
                result.push(reading);
            }
        }

        result
    }

    // fn get_fds(&self) -> PerfEventFds {
    //     let mut result = Vec::new();

    //     for group in &mut self.groups {
    //         if let Ok(fds) = group.fds() {
    //             result.push(fds);
    //         }
    //     }

    //     result
    // }
}
