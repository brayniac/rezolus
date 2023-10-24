use super::stats::*;
use super::*;
use crate::common::Nop;
use metriken::{DynBoxedMetric, MetricBuilder};
use perf_event::events::x86::{Msr, MsrId};
use perf_event::events::Hardware;
use perf_event::{Builder, ReadFormat};
use samplers::hwinfo::hardware_info;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

mod perf_group;
mod proc_cpuinfo;

use perf_group::*;
use proc_cpuinfo::*;

#[distributed_slice(CPU_SAMPLERS)]
fn init(config: &Config) -> Box<dyn Sampler> {
    // try to initialize the perf counter based sampler that provides more info
    // with lower overhead
    if let Ok(perf) = Perf::new(config) {
        Box::new(perf)
    // try to fallback to the /proc/cpuinfo based sampler if perf events are not
    // supported
    } else if let Ok(cpuinfo) = ProcCpuinfo::new(config) {
        Box::new(cpuinfo)
    } else {
        Box::new(Nop {})
    }
}

const NAME: &str = "cpu_perf";

pub struct Perf {
    prev: Instant,
    next: Instant,
    interval: Duration,
    groups: Vec<(SyncSender<()>, Arc<Reading>, std::thread::JoinHandle<()>)>,
    counters: Vec<Vec<DynBoxedMetric<metriken::Counter>>>,
}

impl Perf {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let now = Instant::now();

        let cpus = match hardware_info() {
            Ok(hwinfo) => hwinfo.get_cpus(),
            Err(_) => return Err(()),
        };

        let mut groups = Vec::with_capacity(cpus.len());
        let mut counters = Vec::with_capacity(cpus.len());

        let metrics = [
            "cpu/cycles",
            "cpu/instructions",
            "cpu/ipkc",
            "cpu/ipus",
            "cpu/frequency",
        ];

        for cpu in cpus {
            counters.push(
                metrics
                    .iter()
                    .map(|metric| {
                        MetricBuilder::new(*metric)
                            .metadata("id", format!("{}", cpu.id()))
                            .metadata("core", format!("{}", cpu.core()))
                            .metadata("die", format!("{}", cpu.die()))
                            .metadata("package", format!("{}", cpu.package()))
                            .formatter(cpu_metric_formatter)
                            .build(metriken::Counter::new())
                    })
                    .collect(),
            );

            match PerfGroup::new(cpu.id()) {
                Ok(mut group) => {
                    let (sender, receiver) = std::sync::mpsc::sync_channel(1);

                    let reading = group.reading();
                    let interval = config.interval(NAME);

                    let join_handle = std::thread::spawn(move || {
                        core_affinity::set_for_current(core_affinity::CoreId { id: cpu.id() });

                        loop {
                            let _ = receiver.recv().unwrap();
                            let _ = group.refresh();
                        }
                    });

                    groups.push((sender, reading, join_handle));
                }
                Err(_) => {
                    warn!("Failed to create the perf group on CPU {}", cpu.id());
                    // we want to continue because it's possible that this CPU is offline
                    continue;
                }
            };
        }

        if groups.len() == 0 {
            error!("Failed to create the perf group on any CPU");
            return Err(());
        }

        return Ok(Self {
            prev: now,
            next: now,
            interval: config.interval(NAME),
            groups,
            counters,
        });
    }
}

impl Sampler for Perf {
    fn sample(&mut self) {
        let now = Instant::now();

        if now < self.next {
            return;
        }

        let mut nr_active_groups: u64 = 0;
        let mut total_cycles = 0;
        let mut total_instructions = 0;
        let mut avg_ipkc = 0;
        let mut avg_ipus = 0;
        let mut avg_base_frequency = 0;
        let mut avg_running_frequency = 0;

        for (sender, reading, _) in &mut self.groups {
            sender.send(()).unwrap();

            let id = reading.id.load(Ordering::Relaxed);
            let cycles = reading.cycles.load(Ordering::Relaxed);
            let instructions = reading.instructions.load(Ordering::Relaxed);
            let ipkc = reading.ipkc.load(Ordering::Relaxed);
            let ipus = reading.ipus.load(Ordering::Relaxed);
            let base_frequency_mhz = reading.base_frequency_mhz.load(Ordering::Relaxed);
            let running_frequency_mhz = reading.running_frequency_mhz.load(Ordering::Relaxed);

            nr_active_groups += 1;
            total_cycles += cycles;
            total_instructions += instructions;
            avg_ipkc += ipkc;
            avg_ipus += ipus;
            avg_base_frequency += base_frequency_mhz;
            avg_running_frequency += running_frequency_mhz;

            let _ = CPU_IPKC_HISTOGRAM.increment(ipkc);
            let _ = CPU_IPUS_HISTOGRAM.increment(ipus);
            let _ = CPU_FREQUENCY_HISTOGRAM.increment(running_frequency_mhz);

            self.counters[id][0].set(cycles);
            self.counters[id][1].set(instructions);
            self.counters[id][2].set(ipkc);
            self.counters[id][3].set(ipus);
            self.counters[id][4].set(running_frequency_mhz);
        }

        // we increase the total cycles executed in the last sampling period instead of using the cycle perf event value to handle offlined CPUs.
        CPU_CYCLES.add(total_cycles);
        CPU_INSTRUCTIONS.add(total_instructions);
        CPU_PERF_GROUPS_ACTIVE.set(nr_active_groups as i64);
        CPU_IPKC_AVERAGE.set((avg_ipkc / nr_active_groups) as i64);
        CPU_IPUS_AVERAGE.set((avg_ipus / nr_active_groups) as i64);
        CPU_BASE_FREQUENCY_AVERAGE.set((avg_base_frequency / nr_active_groups) as i64);
        CPU_FREQUENCY_AVERAGE.set((avg_running_frequency / nr_active_groups) as i64);
        CPU_CORES.set(nr_active_groups as _);

        // determine when to sample next
        let next = self.next + self.interval;

        // it's possible we fell behind
        if next > now {
            // if we didn't, sample at the next planned time
            self.next = next;
        } else {
            // if we did, sample after the interval has elapsed
            self.next = now + self.interval;
        }

        // mark when we last sampled
        self.prev = now;
    }
}
