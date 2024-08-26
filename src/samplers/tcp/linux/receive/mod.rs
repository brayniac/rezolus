use crate::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = Receive::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/tcp_receive.bpf.rs"));
}

const NAME: &str = "tcp_receive";

use bpf::*;

use crate::common::bpf::*;
use crate::common::*;
use crate::samplers::tcp::linux::stats::*;

use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

impl GetMap for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "srtt" => &self.maps.srtt,
            "jitter" => &self.maps.jitter,
            _ => unimplemented!(),
        }
    }
}

/// Collects TCP Receive stats using BPF and traces:
/// * `tcp_rcv_established`
///
/// And produces these stats:
/// * `tcp/receive/jitter`
/// * `tcp/receive/srtt`
pub struct Receive {
    thread: JoinHandle<()>,
    notify: Arc<(Mutex<bool>, Condvar)>,
    interval: Interval,
}

impl Receive {
    pub fn new(config: &Config) -> Result<Self, ()> {
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
                    "{NAME} tcp_rcv() BPF instruction count: {}",
                    skel.progs.tcp_rcv_kprobe.insn_cnt()
                );

                // attach the BPF program
                if let Err(e) = skel.attach() {
                    error!("failed to attach bpf program: {e}");
                    return;
                };

                // get the time
                let mut prev = Instant::now();

                // define userspace metric sets

                // wrap the BPF program and define BPF maps
                let mut bpf = BpfBuilder::new(skel)
                    .distribution("srtt", &TCP_SRTT)
                    .distribution("jitter", &TCP_JITTER)
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

                    // refresh userspace metrics
                    bpf.refresh(now.duration_since(prev));

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

        let now = Instant::now();

        Ok(Self {
            thread: handle,
            notify,
            interval: Interval::new(now, config.interval(NAME)),
        })
    }

    pub fn refresh(&mut self, now: Instant) -> Result<(), ()> {
        // early return if it is not time to refresh
        self.interval.try_wait(now)?;

        // check that the thread has not exited
        if self.thread.is_finished() {
            return Err(());
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

        Ok(())
    }
}

#[async_trait]
impl Sampler for Receive {
    async fn sample(&mut self) {
        let now = Instant::now();
        let _ = self.refresh(now);
    }
}
