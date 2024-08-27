use crate::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
    SyscallLatency::init(config)
}

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/syscall_latency.bpf.rs"));
}

const NAME: &str = "syscall_latency";

use bpf::*;

use crate::common::bpf::*;
use crate::samplers::syscall::linux::*;
use crate::samplers::syscall::linux::stats::*;

use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

impl GetMap for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "total_latency" => &self.maps.total_latency,
            "read_latency" => &self.maps.read_latency,
            "write_latency" => &self.maps.write_latency,
            "poll_latency" => &self.maps.poll_latency,
            "lock_latency" => &self.maps.lock_latency,
            "time_latency" => &self.maps.time_latency,
            "sleep_latency" => &self.maps.sleep_latency,
            "socket_latency" => &self.maps.socket_latency,
            "yield_latency" => &self.maps.yield_latency,
            "syscall_lut" => &self.maps.syscall_lut,
            _ => unimplemented!(),
        }
    }
}

/// Collects Scheduler Runqueue Latency stats using BPF and traces:
/// * `raw_syscalls/sys_enter`
/// * `raw_syscalls/sys_exit`
///
/// And produces these stats:
/// * `syscall/total/latency`
/// * `syscall/read/latency`
/// * `syscall/write/latency`
/// * `syscall/poll/latency`
/// * `syscall/lock/latency`
/// * `syscall/time/latency`
/// * `syscall/sleep/latency`
/// * `syscall/socket/latency`
/// * `syscall/yield/latency`
pub struct SyscallLatency {
    thread: JoinHandle<()>,
    notify: Arc<(Mutex<bool>, Condvar)>,
    interval: Interval,
}

impl SyscallLatency {
    pub fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
        // check if sampler should be enabled
        if !(config.enabled(NAME) && config.bpf(NAME)) {
            return Err(());
        }

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
                    "{NAME} sys_enter() BPF instruction count: {}",
                    skel.progs.sys_enter.insn_cnt()
                );
                debug!(
                    "{NAME} sys_exit() BPF instruction count: {}",
                    skel.progs.sys_exit.insn_cnt()
                );

                // attach the BPF program
                if let Err(e) = skel.attach() {
                    error!("failed to attach bpf program: {e}");
                    return;
                };

                // get the time
                let mut prev = Instant::now();

                // generate the syscall LUT
                let syscall_lut = syscall_lut();

                // wrap the BPF program and define BPF maps
                let mut bpf = BpfBuilder::new(skel)
                    .histogram("total_latency", &SYSCALL_TOTAL_LATENCY)
                    .histogram("read_latency", &SYSCALL_READ_LATENCY)
                    .histogram("write_latency", &SYSCALL_WRITE_LATENCY)
                    .histogram("poll_latency", &SYSCALL_POLL_LATENCY)
                    .histogram("lock_latency", &SYSCALL_LOCK_LATENCY)
                    .histogram("time_latency", &SYSCALL_TIME_LATENCY)
                    .histogram("sleep_latency", &SYSCALL_SLEEP_LATENCY)
                    .histogram("socket_latency", &SYSCALL_SOCKET_LATENCY)
                    .histogram("yield_latency", &SYSCALL_YIELD_LATENCY)
                    .map("syscall_lut", &syscall_lut)
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

                    METADATA_SYSCALL_LATENCY_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

                    // refresh userspace metrics
                    bpf.refresh(now.duration_since(prev));

                    let elapsed = now.elapsed().as_nanos() as u64;
                    METADATA_SYSCALL_LATENCY_RUNTIME.add(elapsed);
                    let _ = METADATA_SYSCALL_LATENCY_RUNTIME_HISTOGRAM.increment(elapsed);

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
impl Sampler for SyscallLatency {
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
