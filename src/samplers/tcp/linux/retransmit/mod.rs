use crate::*;

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(s) = Retransmit::new(config) {
        Some(Box::new(s))
    } else {
        None
    }
}

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/tcp_retransmit.bpf.rs"));
}

const NAME: &str = "tcp_retransmit";

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
            "counters" => &self.maps.counters,
            _ => unimplemented!(),
        }
    }
}

/// Collects TCP Retransmit stats using BPF and traces:
/// * `tcp_retransmit_timer`
///
/// And produces these stats:
/// * `tcp/transmit/retransmit`
pub struct Retransmit {
    thread: JoinHandle<()>,
    notify: Arc<(Mutex<bool>, Condvar)>,
    interval: Interval,
}

impl Retransmit {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !(config.enabled(NAME) && config.bpf(NAME)) {
            return Err(());
        }

        // create vars to communicate with our child thread
        let initialized = Arc::new(AtomicBool::new(false));
        let notify = Arc::new((Mutex::new(false), Condvar::new()));

        // define userspace metric sets
        let counters = vec![CounterWithHist::new(
            &TCP_TX_RETRANSMIT,
            &TCP_TX_RETRANSMIT_HISTOGRAM,
        )];

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
                    "{NAME} tcp_retransmit_skb() BPF instruction count: {}",
                    skel.progs.tcp_retransmit_skb.insn_cnt()
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
                let mut bpf = BpfBuilder::new(skel).counters("counters", counters).build();

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

                    METADATA_TCP_RETRANSMIT_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

                    // refresh userspace metrics
                    bpf.refresh(now.duration_since(prev));

                    let elapsed = now.elapsed().as_nanos() as u64;
                    METADATA_TCP_RETRANSMIT_RUNTIME.add(elapsed);
                    let _ = METADATA_TCP_RETRANSMIT_RUNTIME_HISTOGRAM.increment(elapsed);

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

        Ok(Self {
            thread: handle,
            notify,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for Retransmit {
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
