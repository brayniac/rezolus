use crate::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    Runqlat::init(config)
}

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/scheduler_runqueue.bpf.rs"));
}

const NAME: &str = "scheduler_runqueue";

use bpf::*;

use crate::common::bpf::*;
use crate::common::*;
use crate::samplers::scheduler::stats::*;

use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

impl GetMap for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "counters" => &self.maps.counters,
            "runqlat" => &self.maps.runqlat,
            "running" => &self.maps.running,
            "offcpu" => &self.maps.offcpu,
            _ => unimplemented!(),
        }
    }
}

/// Collects Scheduler Runqueue Latency stats using BPF and traces:
/// * `sched_wakeup`
/// * `sched_wakeup_new`
/// * `sched_switch`
///
/// And produces these stats:
/// * `scheduler/runqueue/latency`
/// * `scheduler/running`
/// * `scheduler/context_switch/involuntary`
/// * `scheduler/context_switch/voluntary`
pub struct Runqlat {
    thread: JoinHandle<()>,
    notify: Arc<(Mutex<bool>, Condvar)>,
    interval: Interval,
}

impl Runqlat {
    pub fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
        // check if sampler should be enabled
        if !(config.enabled(NAME) && config.bpf(NAME)) {
            return Err(());
        }

        // define userspace metric sets
        let counters = vec![CounterWithHist::new(
            &SCHEDULER_IVCSW,
            &SCHEDULER_IVCSW_HISTOGRAM,
        )];

        // create vars to communicate with our child thread
        let initialized = Arc::new(AtomicBool::new(false));
        let notify = Arc::new((Mutex::new(false), Condvar::new()));

        // create a child thread which owns the BPF sampler
        let handle = {
            let initialized = initialized.clone();
            let notify = notify.clone();

            std::thread::spawn(move || {
                // storage for the BPF object file
                let open_object: &'static mut MaybeUninit<OpenObject> =
                    Box::leak(Box::new(MaybeUninit::uninit()));

                // open and load the program
                let mut skel = match ModSkelBuilder::default().open(open_object) {
                    Ok(s) => match s.load() {
                        Ok(s) => s,
                        Err(e) => {
                            error!("failed to load bpf program: {e}");
                            return;
                        }
                    },
                    Err(e) => {
                        error!("failed to open bpf builder: {e}");
                        return;
                    }
                };

                // debugging info about BPF instruction counts
                debug!(
                    "{NAME} handle__sched_wakeup() BPF instruction count: {}",
                    skel.progs.handle__sched_wakeup.insn_cnt()
                );
                debug!(
                    "{NAME} handle__sched_wakeup_new() BPF instruction count: {}",
                    skel.progs.handle__sched_wakeup_new.insn_cnt()
                );
                debug!(
                    "{NAME} handle__sched_switch() BPF instruction count: {}",
                    skel.progs.handle__sched_switch.insn_cnt()
                );

                // attach the BPF program
                if let Err(e) = skel.attach() {
                    error!("failed to attach bpf program: {e}");
                    return;
                };

                // get the time
                let mut prev = Instant::now();

                // wrap the BPF program and define BPF maps
                let mut bpf = BpfBuilder::new(skel)
                    .counters("counters", counters)
                    .histogram("runqlat", &SCHEDULER_RUNQUEUE_LATENCY)
                    .histogram("running", &SCHEDULER_RUNNING)
                    .histogram("offcpu", &SCHEDULER_OFFCPU)
                    .build();

                // indicate that we have completed initialization
                initialized.store(true, Ordering::SeqCst);

                // the sampler loop
                loop {
                    // wait until we are notified to start
                    {
                        let &(ref lock, ref cvar) = &*notify;
                        let mut started = lock.lock();
                        if !*started {
                            cvar.wait(&mut started);
                        }
                    }

                    let now = Instant::now();

                    METADATA_SCHEDULER_RUNQUEUE_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

                    // refresh userspace metrics
                    bpf.refresh(now.duration_since(prev));

                    let elapsed = now.elapsed().as_nanos() as u64;
                    METADATA_SCHEDULER_RUNQUEUE_RUNTIME.add(elapsed);
                    let _ = METADATA_SCHEDULER_RUNQUEUE_RUNTIME_HISTOGRAM.increment(elapsed);

                    prev = now;

                    // notify that we have finished running
                    {
                        let &(ref lock, ref cvar) = &*notify;
                        let mut running = lock.lock();
                        *running = false;
                        cvar.notify_one();
                    }
                }
            })
        };

        // block waiting for initialization
        while !handle.is_finished() || !initialized.load(Ordering::SeqCst) {
            std::thread::sleep(core::time::Duration::from_millis(1));
        }

        // if the thread has terminated, there was an error loading the sampler
        if handle.is_finished() {
            return Err(());
        }

        Ok(Box::new(Self {
            thread: handle,
            notify,
            interval: config.interval(NAME),
        }))
    }
}

#[async_trait]
impl Sampler for Runqlat {
    async fn sample(&mut self) {
        // wait until it's time to sample
        self.interval.tick().await;

        // check that the thread has not exited
        if self.thread.is_finished() {
            return;
        }

        // notify the thread to start
        {
            let &(ref lock, ref cvar) = &*self.notify;
            let mut started = lock.lock();
            *started = true;
            cvar.notify_one();
        }

        // wait for notification that thread has finished
        {
            let &(ref lock, ref cvar) = &*self.notify;
            let mut running = lock.lock();
            if *running {
                cvar.wait(&mut running);
            }
        }
    }

    fn is_fast(&self) -> bool {
        true
    }
}
