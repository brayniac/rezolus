#[distributed_slice(SCHEDULER_SAMPLERS)]
fn init(config: &Config) -> Box<dyn Sampler> {
    if let Ok(s) = Runqlat::new(config) {
        Box::new(s)
    } else {
        Box::new(Nop {})
    }
}

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/scheduler_runqueue.bpf.rs"));
}

const NAME: &str = "scheduler_runqueue";

// use metriken::MetricBuilder;
use metriken::MetricBuilder;
use metriken::DynBoxedMetric;
use metriken::RwLockHistogram;
use std::collections::HashSet;
use memmap2::MmapMut;
use bpf::*;

use crate::common::bpf::*;
use crate::common::*;
use crate::samplers::scheduler::stats::*;
use crate::samplers::scheduler::*;

use std::os::fd::{AsFd, AsRawFd, FromRawFd};

impl GetMap for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        self.obj.map(name).unwrap()
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
    bpf: Bpf<ModSkel<'static>>,
    pid_lut: MmapMut,
    pid_groups: HashMap<String, PidGroup>,
    counter_interval: Duration,
    counter_next: Instant,
    counter_prev: Instant,
    distribution_interval: Duration,
    distribution_next: Instant,
    distribution_prev: Instant,
}

pub struct PidGroup {
    name: String,
    pids: HashSet<u32>,
    offcpu: Arc<DynBoxedMetric<RwLockHistogram>>,
    running: Arc<DynBoxedMetric<RwLockHistogram>>,
    runqlat: Arc<DynBoxedMetric<RwLockHistogram>>,
}

impl Runqlat {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let builder = ModSkelBuilder::default();
        let mut skel = builder
            .open()
            .map_err(|e| error!("failed to open bpf builder: {e}"))?
            .load()
            .map_err(|e| error!("failed to load bpf program: {e}"))?;

        debug!(
            "{NAME} handle__sched_wakeup() BPF instruction count: {}",
            skel.progs().handle__sched_wakeup().insn_cnt()
        );
        debug!(
            "{NAME} handle__sched_wakeup_new() BPF instruction count: {}",
            skel.progs().handle__sched_wakeup_new().insn_cnt()
        );
        debug!(
            "{NAME} handle__sched_switch() BPF instruction count: {}",
            skel.progs().handle__sched_switch().insn_cnt()
        );

        skel.attach()
            .map_err(|e| error!("failed to attach bpf program: {e}"))?;

        let mut bpf = Bpf::from_skel(skel);

        let fd = bpf.map("pid_lut").as_fd().as_raw_fd();
        let file = unsafe { std::fs::File::from_raw_fd(fd as _) };
        let pid_lut = unsafe {
            memmap2::MmapOptions::new()
                .len(4194304)
                .map_mut(&file)
                .expect("failed to mmap() bpf pid lut")
        };

        let counters = vec![Counter::new(&SCHEDULER_IVCSW, None)];

        bpf.add_counters("counters", counters);

        let mut distributions = vec![
            ("runqlat", &SCHEDULER_RUNQUEUE_LATENCY),
            ("running", &SCHEDULER_RUNNING),
            ("offcpu", &SCHEDULER_OFFCPU),
        ];

        for (name, histogram) in distributions.drain(..) {
            bpf.add_distribution(name, histogram);
        }

        bpf.add_multi_distribution("runqlat_grouped", histogram::Config::new(5, 64).unwrap(), 8).unwrap();
        bpf.add_multi_distribution("running_grouped", histogram::Config::new(5, 64).unwrap(), 8).unwrap();
        bpf.add_multi_distribution("offcpu_grouped", histogram::Config::new(5, 64).unwrap(), 8).unwrap();

        Ok(Self {
            bpf,
            pid_lut,
            pid_groups: HashMap::new(),
            counter_interval: config.interval(NAME),
            counter_next: Instant::now(),
            counter_prev: Instant::now(),
            distribution_interval: config.distribution_interval(NAME),
            distribution_next: Instant::now(),
            distribution_prev: Instant::now(),
        })
    }

    pub fn refresh_counters(&mut self, now: Instant) {
        if now < self.counter_next {
            return;
        }

        let elapsed = (now - self.counter_prev).as_secs_f64();

        self.bpf.refresh_counters(elapsed);

        // determine when to sample next
        let next = self.counter_next + self.counter_interval;

        // check that next sample time is in the future
        if next > now {
            self.counter_next = next;
        } else {
            self.counter_next = now + self.counter_interval;
        }

        // mark when we last sampled
        self.counter_prev = now;
    }

    pub fn refresh_distributions(&mut self, now: Instant) {
        if now < self.distribution_next {
            return;
        }

        self.bpf.refresh_distributions();

        // determine when to sample next
        let next = self.distribution_next + self.distribution_interval;

        // check that next sample time is in the future
        if next > now {
            self.distribution_next = next;
        } else {
            self.distribution_next = now + self.distribution_interval;
        }

        // mark when we last sampled
        self.distribution_prev = now;
    }
}

impl Sampler for Runqlat {
    fn sample(&mut self) {
        let now = Instant::now();
        self.refresh_counters(now);
        self.refresh_distributions(now);
    }

    fn register_pid_group(&mut self, name: &str, index: usize) -> Result<(), ()> {
        if self.pid_groups.contains_key(name) {
            error!("pid group already defined");
            return Err(());
        }

        let runqlat = Arc::new(MetricBuilder::new("runqlat")
            .metadata("group", name.to_string())
            .formatter(scheduler_metric_formatter)
            .build(metriken::RwLockHistogram::new(5, 64)));

        self.bpf.add_to_multi_distribution("runqlat_grouped", index, runqlat.clone()).unwrap();

        let running = Arc::new(MetricBuilder::new("running")
            .metadata("group", name.to_string())
            .formatter(scheduler_metric_formatter)
            .build(metriken::RwLockHistogram::new(5, 64)));

        self.bpf.add_to_multi_distribution("running_grouped", index, running.clone()).unwrap();

        let offcpu = Arc::new(MetricBuilder::new("offcpu")
            .metadata("group", name.to_string())
            .formatter(scheduler_metric_formatter)
            .build(metriken::RwLockHistogram::new(5, 64)));

        self.bpf.add_to_multi_distribution("offcpu_grouped", index, offcpu.clone()).unwrap();


        let pid_group = PidGroup {
            name: name.to_string(),
            pids: HashSet::new(),
            runqlat,
            running,
            offcpu,
        };

        self.pid_groups.insert(name.to_string(), pid_group);

        Ok(())
    }
}
