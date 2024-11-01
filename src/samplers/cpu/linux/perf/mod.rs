const NAME: &str = "cpu_perf";

use crate::common::perf_events::*;
use crate::common::*;
use crate::samplers::cpu::linux::stats::*;
use crate::samplers::cpu::stats::*;
use crate::samplers::Sampler;
use crate::*;

use tokio::task::spawn_blocking;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let s = Perf::new()?;

    Ok(Some(Box::new(s)))
}

pub struct Perf {
    counters: ScopedCounters,
    gauges: ScopedGauges,
}

impl Perf {
    pub fn new() -> Result<Self, std::io::Error> {
        let cpus = common::linux::cpus()?;

        let mut counters = ScopedCounters::new();
        let mut gauges = ScopedGauges::new();

        for cpu in cpus {
            for counter in &["cpu/cycles", "cpu/instructions"] {
                counters.push(
                    cpu,
                    DynamicCounterBuilder::new(*counter)
                        .metadata("id", format!("{}", cpu))
                        .formatter(cpu_metric_formatter)
                        .build(),
                );
            }

            for gauge in &["cpu/ipkc", "cpu/ipus", "cpu/frequency"] {
                gauges.push(
                    cpu,
                    DynamicGaugeBuilder::new(*gauge)
                        .metadata("id", format!("{}", cpu))
                        .formatter(cpu_metric_formatter)
                        .build(),
                );
            }
        }

        {
            let groups = PERF_GROUPS.blocking_lock();

            if groups.len() == 0 {
                error!("No perf event groups have been initialized");
            }
        }

        Ok(Self { counters, gauges })
    }
}

#[async_trait]
impl Sampler for Perf {
    async fn refresh(&self) {
        let mut nr_active_groups: u64 = 0;

        let mut avg_ipkc = 0;
        let mut avg_ipus = 0;
        let mut avg_base_frequency = 0;
        let mut avg_running_frequency = 0;

        let readings = PERF_EVENTS.lock().await.read().await;

        for reading in readings {
            nr_active_groups += 1;

            avg_ipkc += reading.ipkc.unwrap_or(0);
            avg_ipus += reading.ipus.unwrap_or(0);
            avg_base_frequency += reading.base_frequency_mhz.unwrap_or(0);
            avg_running_frequency += reading.running_frequency_mhz.unwrap_or(0);

            // note: add counters, these are deltas
            if let Some(c) = reading.cycles {
                let _ = self.counters.add(reading.cpu, 0, c);
                CPU_CYCLES.add(c);
            }
            if let Some(c) = reading.instructions {
                let _ = self.counters.add(reading.cpu, 1, c);
                CPU_INSTRUCTIONS.add(c);
            }

            if let Some(g) = reading.ipkc {
                let _ = self.gauges.set(reading.cpu, 0, g as _);
            }
            if let Some(g) = reading.ipus {
                let _ = self.gauges.set(reading.cpu, 1, g as _);
            }
            if let Some(g) = reading.running_frequency_mhz {
                let _ = self.gauges.set(reading.cpu, 2, g as _);
            }
        }

        // we can only update averages if at least one group of perf
        // counters was active in the period
        if nr_active_groups > 0 {
            CPU_PERF_GROUPS_ACTIVE.set(nr_active_groups as i64);
            CPU_IPKC_AVERAGE.set((avg_ipkc / nr_active_groups) as i64);
            CPU_IPUS_AVERAGE.set((avg_ipus / nr_active_groups) as i64);
            CPU_BASE_FREQUENCY_AVERAGE.set((avg_base_frequency / nr_active_groups) as i64);
            CPU_FREQUENCY_AVERAGE.set((avg_running_frequency / nr_active_groups) as i64);
        }
    }
}
