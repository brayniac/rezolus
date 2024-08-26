use crate::*;

// use crate::common::{Counter, Interval};
use crate::samplers::cpu::*;
use crate::samplers::hwinfo::hardware_info;
use metriken::DynBoxedMetric;
use metriken::MetricBuilder;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::NAME;

const CPU_IDLE_FIELD_INDEX: usize = 3;
const CPU_IO_WAIT_FIELD_INDEX: usize = 4;

pub struct ProcStat {
    interval: Interval,
    nanos_per_tick: u64,
    file: File,
    total_counters: Vec<CounterWithHist>,
    total_busy: CounterWithHist,
    percpu_counters: Vec<Vec<DynBoxedMetric<metriken::Counter>>>,
    percpu_busy: Vec<DynBoxedMetric<metriken::Counter>>,
}

impl ProcStat {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let cpus = match hardware_info() {
            Ok(hwinfo) => hwinfo.get_cpus(),
            Err(_) => return Err(()),
        };

        let total_counters = vec![
            CounterWithHist::new(&CPU_USAGE_USER, &CPU_USAGE_USER_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_NICE, &CPU_USAGE_NICE_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_SYSTEM, &CPU_USAGE_SYSTEM_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_IDLE, &CPU_USAGE_IDLE_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_IO_WAIT, &CPU_USAGE_IO_WAIT_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_IRQ, &CPU_USAGE_IRQ_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_SOFTIRQ, &CPU_USAGE_SOFTIRQ_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_STEAL, &CPU_USAGE_STEAL_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_GUEST, &CPU_USAGE_GUEST_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_GUEST_NICE, &CPU_USAGE_GUEST_NICE_HISTOGRAM),
        ];

        let mut percpu_counters = Vec::with_capacity(cpus.len());
        let mut percpu_busy = Vec::new();

        for cpu in cpus {
            let states = [
                "user",
                "nice",
                "system",
                "idle",
                "io_wait",
                "irq",
                "softirq",
                "steal",
                "guest",
                "guest_nice",
            ];

            let counters: Vec<DynBoxedMetric<metriken::Counter>> = states
                .iter()
                .map(|state| {
                    MetricBuilder::new("cpu/usage")
                        .metadata("id", format!("{}", cpu.id()))
                        .metadata("core", format!("{}", cpu.core()))
                        .metadata("die", format!("{}", cpu.die()))
                        .metadata("package", format!("{}", cpu.package()))
                        .metadata("state", *state)
                        .formatter(cpu_metric_formatter)
                        .build(metriken::Counter::new())
                })
                .collect();

            percpu_counters.push(counters);

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

        let sc_clk_tck =
            sysconf::raw::sysconf(sysconf::raw::SysconfVariable::ScClkTck).map_err(|_| {
                error!("Failed to get system clock tick rate");
            })?;

        let nanos_per_tick = 1_000_000_000 / (sc_clk_tck as u64);

        let file = std::fs::File::open("/proc/stat")
            .map(|f| File::from_std(f))
            .map_err(|e| {
                error!("failed to open /proc/stat: {e}");
            })?;

        Ok(Self {
            file,
            total_counters,
            total_busy: CounterWithHist::new(&CPU_USAGE_BUSY, &CPU_USAGE_BUSY_HISTOGRAM),
            percpu_counters,
            percpu_busy,
            nanos_per_tick,
            interval: config.interval(NAME),
        })
    }
}

#[async_trait]
impl Sampler for ProcStat {
    async fn sample(&mut self) {
        let elapsed = self.interval.tick().await;
        let _ = self.sample_proc_stat(elapsed);
    }
}

impl ProcStat {
    async fn sample_proc_stat(&mut self, elapsed: Option<Duration>) -> Result<(), std::io::Error> {
        self.file.rewind().await?;

        let mut data = String::new();
        self.file.read_to_string(&mut data).await?;

        let lines = data.lines();

        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.is_empty() {
                continue;
            }

            let header = parts.first().unwrap();

            if *header == "cpu" {
                let mut busy: u64 = 0;

                for (field, counter) in self.total_counters.iter_mut().enumerate() {
                    if let Some(Ok(v)) = parts.get(field + 1).map(|v: &&str| v.parse::<u64>()) {
                        if field != CPU_IDLE_FIELD_INDEX && field != CPU_IO_WAIT_FIELD_INDEX {
                            busy = busy.wrapping_add(v);
                        }
                        counter.set(elapsed, v.wrapping_mul(self.nanos_per_tick));
                    }

                    self.total_busy
                        .set(elapsed, busy.wrapping_mul(self.nanos_per_tick));
                }
            } else if header.starts_with("cpu") {
                if let Ok(id) = header.replace("cpu", "").parse::<usize>() {
                    let mut busy: u64 = 0;

                    for (field, counter) in self.percpu_counters[id].iter_mut().enumerate() {
                        if let Some(Ok(v)) = parts.get(field + 1).map(|v| v.parse::<u64>()) {
                            if field != CPU_IDLE_FIELD_INDEX && field != CPU_IO_WAIT_FIELD_INDEX {
                                busy = busy.wrapping_add(v);
                            }
                            counter.set(v.wrapping_mul(self.nanos_per_tick));
                        }
                    }

                    self.percpu_busy[id].set(busy.wrapping_mul(self.nanos_per_tick));
                }
            }
        }

        Ok(())
    }
}
