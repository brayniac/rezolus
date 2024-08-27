use crate::*;

#[allow(clippy::module_inception)]
mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_usage.bpf.rs"));
}

use super::NAME;

use metriken::{DynBoxedMetric, MetricBuilder};

use bpf::*;

use crate::common::bpf::*;
use crate::common::*;
use crate::samplers::cpu::*;
use crate::samplers::hwinfo::hardware_info;

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

/// Collects CPU Usage stats using BPF and traces:
/// * __cgroup_account_cputime_field
///
/// And produces these stats:
/// * cpu/usage/*

pub struct CpuUsage {
    thread: JoinHandle<()>,
    notify: Arc<(Mutex<bool>, Condvar)>,
    interval: Interval,
    percpu_counters: Arc<PercpuCounters>,
    total_busy: CounterWithHist,
    percpu_busy: Vec<DynBoxedMetric<metriken::Counter>>,
}

impl CpuUsage {
    pub fn init(config: &Config) -> Result<Box<dyn Sampler>, ()> {
        // check if sampler should be enabled
        if !(config.enabled(NAME) && config.bpf(NAME)) {
            return Err(());
        }

        // define userspace metric sets

        let cpus = match hardware_info() {
            Ok(hwinfo) => hwinfo.get_cpus(),
            Err(_) => return Err(()),
        };

        let counters = vec![
            CounterWithHist::new(&CPU_USAGE_USER, &CPU_USAGE_USER_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_NICE, &CPU_USAGE_NICE_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_SYSTEM, &CPU_USAGE_SYSTEM_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_SOFTIRQ, &CPU_USAGE_SOFTIRQ_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_IRQ, &CPU_USAGE_IRQ_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_STEAL, &CPU_USAGE_STEAL_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_GUEST, &CPU_USAGE_GUEST_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_GUEST_NICE, &CPU_USAGE_GUEST_NICE_HISTOGRAM),
        ];

        let mut percpu_counters = PercpuCounters::default();
        let mut percpu_busy = Vec::new();

        let states = [
            "user",
            "nice",
            "system",
            "softirq",
            "irq",
            "steal",
            "guest",
            "guest_nice",
        ];

        for cpu in cpus {
            for state in states {
                percpu_counters.push(
                    cpu.id(),
                    MetricBuilder::new("cpu/usage")
                        .metadata("id", format!("{}", cpu.id()))
                        .metadata("core", format!("{}", cpu.core()))
                        .metadata("die", format!("{}", cpu.die()))
                        .metadata("package", format!("{}", cpu.package()))
                        .metadata("state", state)
                        .formatter(cpu_metric_formatter)
                        .build(metriken::Counter::new()),
                );
            }
            percpu_busy.push(
                MetricBuilder::new("cpu/usage")
                    .metadata("id", format!("{}", cpu.id()))
                    .metadata("core", format!("{}", cpu.core()))
                    .metadata("die", format!("{}", cpu.die()))
                    .metadata("package", format!("{}", cpu.package()))
                    .metadata("state", "busy")
                    .formatter(cpu_metric_formatter)
                    .build(metriken::Counter::new()),
            );
        }

        let percpu_counters = Arc::new(percpu_counters);

        // create vars to communicate with our child thread
        let initialized = Arc::new(AtomicBool::new(false));
        let notify = Arc::new((Mutex::new(false), Condvar::new()));

        // create a child thread which owns the BPF sampler
        let handle = {
            let initialized = initialized.clone();
            let notify = notify.clone();

            let percpu_counters = percpu_counters.clone();

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

                // attach the BPF program
                if let Err(e) = skel.attach() {
                    error!("failed to attach bpf program: {e}");
                    return;
                };

                // get the time
                let mut prev = Instant::now();

                // wrap the BPF program and define BPF maps
                let mut bpf = BpfBuilder::new(skel)
                    .percpu_counters("counters", counters, percpu_counters)
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

                    METADATA_CPU_USAGE_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

                    // refresh userspace metrics
                    bpf.refresh(now.duration_since(prev));

                    let elapsed = now.elapsed().as_nanos() as u64;
                    METADATA_CPU_USAGE_RUNTIME.add(elapsed);
                    let _ = METADATA_CPU_USAGE_RUNTIME_HISTOGRAM.increment(elapsed);

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

        let total_busy = CounterWithHist::new(&CPU_USAGE_BUSY, &CPU_USAGE_BUSY_HISTOGRAM);

        Ok(Box::new(Self {
            thread: handle,
            notify,
            interval: config.interval(NAME),
            total_busy,
            percpu_counters,
            percpu_busy,
        }))
    }
}

fn busy() -> u64 {
    [
        &CPU_USAGE_USER,
        &CPU_USAGE_NICE,
        &CPU_USAGE_SYSTEM,
        &CPU_USAGE_SOFTIRQ,
        &CPU_USAGE_IRQ,
        &CPU_USAGE_STEAL,
        &CPU_USAGE_GUEST,
        &CPU_USAGE_GUEST_NICE,
    ]
    .iter()
    .map(|v| v.value())
    .sum()
}

#[async_trait]
impl Sampler for CpuUsage {
    async fn sample(&mut self) {
        // wait until it's time to sample
        let elapsed = self.interval.tick().await;

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

        // update busy time metric
        let busy: u64 = busy();
        let _ = self.total_busy.set(elapsed, busy);

        // do the same for percpu counters
        for (cpu, busy_counter) in self.percpu_busy.iter().enumerate() {
            let busy: u64 = self.percpu_counters.sum(cpu).unwrap_or(0);
            let _ = busy_counter.set(busy);
        }
    }

    fn is_fast(&self) -> bool {
        true
    }
}
