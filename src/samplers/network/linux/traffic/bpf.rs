#[allow(clippy::module_inception)]
mod bpf {
    include!(concat!(env!("OUT_DIR"), "/network_traffic.bpf.rs"));
}

use super::NAME;

use crate::*;

use bpf::*;

use crate::common::bpf::*;
use crate::common::*;
use crate::samplers::network::stats::*;

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

/// Collects Network Traffic stats using BPF and traces:
/// * `netif_receive_skb`
/// * `netdev_start_xmit`
///
/// And produces these stats:
/// * `network/receive/bytes`
/// * `network/receive/frames`
/// * `network/transmit/bytes`
/// * `network/transmit/frames`
pub struct NetworkTraffic {
    thread: JoinHandle<()>,
    notify: Arc<(Mutex<bool>, Condvar)>,
    interval: Interval,
}

impl NetworkTraffic {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !(config.enabled(NAME) && config.bpf(NAME)) {
            return Err(());
        }

        // create vars to communicate with our child thread
        let initialized = Arc::new(AtomicBool::new(false));
        let notify = Arc::new((Mutex::new(false), Condvar::new()));

        // define userspace metric sets
        let counters = vec![
            Counter::new(&NETWORK_RX_BYTES, Some(&NETWORK_RX_BYTES_HISTOGRAM)),
            Counter::new(&NETWORK_TX_BYTES, Some(&NETWORK_TX_BYTES_HISTOGRAM)),
            Counter::new(&NETWORK_RX_PACKETS, Some(&NETWORK_RX_PACKETS_HISTOGRAM)),
            Counter::new(&NETWORK_TX_PACKETS, Some(&NETWORK_TX_PACKETS_HISTOGRAM)),
        ];

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
                    "{NAME} netif_receive_skb() BPF instruction count: {}",
                    skel.progs.netif_receive_skb.insn_cnt()
                );
                debug!(
                    "{NAME} tcp_cleanup_rbuf() BPF instruction count: {}",
                    skel.progs.tcp_cleanup_rbuf.insn_cnt()
                );

                // attach the BPF program
                if let Err(e) = skel.attach() {
                    error!("failed to attach bpf program: {e}");
                    return;
                };

                // get the time
                let mut prev = Instant::now();

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

                    // refresh userspace metrics
                    bpf.refresh_counters(now.duration_since(prev));
                    bpf.refresh_distributions();

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
impl Sampler for NetworkTraffic {
    async fn sample(&mut self) {
        let now = Instant::now();
        let _ = self.refresh(now);
    }
}
