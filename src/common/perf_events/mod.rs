use crate::common;
use crate::common::SyncPrimitive;
use crate::*;

use tokio::sync::Mutex;
use tokio::sync::mpsc::*;

use std::collections::HashMap;
use std::sync::LazyLock;
use std::os::fd::RawFd;

mod counter;
mod group;

pub use counter::Counter;
pub use group::Reading;

use group::PerfGroup;

pub static PERF_EVENTS: LazyLock<Mutex<PerfEvents>> =
    LazyLock::new(|| Mutex::new(PerfEvents::new()));

pub struct PerfEvents {
    thread: std::thread::JoinHandle<Result<(), libbpf_rs::Error>>,
    sync: SyncPrimitive,
    rx: Receiver<Vec<Reading>>,
    fds: Arc<PerfEventFds>,
}

pub struct PerfEventFds {
    inner: HashMap<usize, HashMap<usize, RawFd>>,
}

impl PerfEventFds {
    pub fn get(&self, cpu: usize, counter: Counter) -> Option<RawFd> {
        if let Some(g) = self.inner.get(&cpu) {
            g.get(&(counter as usize)).copied()
        } else {
            None
        }
    }
}

impl PerfEvents {
    pub fn new() -> Self {
        let sync = SyncPrimitive::new();
        let sync2 = sync.clone();

        let mut groups = PerfGroups::new();

        let (tx, rx) = channel(100);

        let fds = groups.file_descriptors();

        let thread = std::thread::spawn(move || {
            // the sampling loop
            loop {
                // blocking wait until we are notified to start, no cpu consumed
                sync.wait_trigger();

                // get the readings and send them on the queue
                let readings = groups.readings();
                let _ = tx.try_send(readings);

                // notify that we have finished running
                sync.notify();
            }
        });

        Self {
            thread,
            sync: sync2,
            rx,
            fds: fds.into(),
        }
    }

    pub async fn read(&mut self) -> Vec<Reading> {
        // check that the thread has not exited
        if self.thread.is_finished() {
            panic!("thread exited early");
        }

        // notify the thread to start
        self.sync.trigger();

        // wait for notification that thread has finished
        self.sync.wait_notify().await;

        // get the readings from the queue
        self.rx.recv().await.expect("failed to get perf readings")
    }

    pub fn file_descriptors(&self) -> Arc<PerfEventFds> {
        self.fds.clone()
    }
}



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
            if let Some(group) = group {
                if let Ok(reading) = group.get_metrics() {
                    result.push(reading);
                }
            }
        }

        result
    }

    /// Collect readings from all of the groups.
    pub fn file_descriptors(&mut self) -> PerfEventFds {
        let mut inner = HashMap::new();

        for (cpu, group) in self.groups.iter_mut().enumerate() {
            if let Some(group) = group {
                inner.insert(cpu, group.file_descriptors());
            }
        }

        PerfEventFds {
            inner,
        }
    }
}
