use crate::common::Interval;
use crate::samplers::cpu::*;
use crate::*;
use crate::{distributed_slice, Config, Sampler};
use libc::mach_port_t;
use metriken::{DynBoxedMetric, MetricBuilder};
use ringlog::error;

const NAME: &str = "cpu_usage";

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>, runtime: &Runtime) {
    runtime.spawn(async {
        if let Ok(mut s) = CpuUsage::init(config) {
            loop {
                s.sample().await;
            }
        }
    });
}

struct CpuUsage {
    interval: Interval,
    port: mach_port_t,
    nanos_per_tick: u64,
    counters_total: Vec<CounterWithHist>,
    counters_percpu: Vec<Vec<DynBoxedMetric<metriken::Counter>>>,
}

impl CpuUsage {
    pub fn init(config: Arc<Config>) -> Result<Box<dyn Sampler>, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        let cpus = num_cpus::get();

        let counters_total = vec![
            CounterWithHist::new(&CPU_USAGE_USER, &CPU_USAGE_USER_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_NICE, &CPU_USAGE_NICE_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_SYSTEM, &CPU_USAGE_SYSTEM_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_IDLE, &CPU_USAGE_IDLE_HISTOGRAM),
            CounterWithHist::new(&CPU_USAGE_BUSY, &CPU_USAGE_BUSY_HISTOGRAM),
        ];

        let mut counters_percpu = Vec::with_capacity(cpus);

        for cpu in 0..cpus {
            let states = ["user", "nice", "system", "idle", "busy"];

            let counters: Vec<DynBoxedMetric<metriken::Counter>> = states
                .iter()
                .map(|state| {
                    MetricBuilder::new("cpu/usage")
                        .metadata("id", format!("{}", cpu))
                        .metadata("state", *state)
                        .formatter(cpu_metric_formatter)
                        .build(metriken::Counter::new())
                })
                .collect();

            counters_percpu.push(counters);
        }

        let sc_clk_tck =
            sysconf::raw::sysconf(sysconf::raw::SysconfVariable::ScClkTck).map_err(|_| {
                error!("Failed to get system clock tick rate");
            })?;

        let nanos_per_tick = 1_000_000_000 / (sc_clk_tck as u64);

        Ok(Box::new(Self {
            interval: config.interval(NAME),
            port: unsafe { libc::mach_host_self() },
            nanos_per_tick,
            counters_total,
            counters_percpu,
        }))
    }
}

#[async_trait]
impl Sampler for CpuUsage {
    async fn sample(&mut self) {
        let elapsed = self.interval.tick().await;

        let now = Instant::now();
        METADATA_CPU_USAGE_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());

        unsafe {
            let _ = self.sample_processor_info(elapsed);
        }

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_CPU_USAGE_RUNTIME.add(elapsed);
        let _ = METADATA_CPU_USAGE_RUNTIME_HISTOGRAM.increment(elapsed);
    }
}

impl CpuUsage {
    unsafe fn sample_processor_info(
        &mut self,
        elapsed: Option<Duration>,
    ) -> Result<(), std::io::Error> {
        let mut num_cpu: u32 = 0;
        let mut cpu_info: *mut i32 = std::ptr::null_mut();
        let mut cpu_info_len: u32 = 0;

        let mut total_user = 0;
        let mut total_system = 0;
        let mut total_idle = 0;
        let mut total_nice = 0;
        let mut total_busy = 0;

        if libc::host_processor_info(
            self.port,
            libc::PROCESSOR_CPU_LOAD_INFO,
            &mut num_cpu as *mut u32,
            &mut cpu_info as *mut *mut i32,
            &mut cpu_info_len as *mut u32,
        ) == libc::KERN_SUCCESS
        {
            for cpu in 0..num_cpu {
                let user = (*cpu_info
                    .offset((cpu as i32 * libc::CPU_STATE_MAX + libc::CPU_STATE_USER) as isize)
                    as u64)
                    .wrapping_mul(self.nanos_per_tick);
                let system = (*cpu_info
                    .offset((cpu as i32 * libc::CPU_STATE_MAX + libc::CPU_STATE_SYSTEM) as isize)
                    as u64)
                    .wrapping_mul(self.nanos_per_tick);
                let idle = (*cpu_info
                    .offset((cpu as i32 * libc::CPU_STATE_MAX + libc::CPU_STATE_IDLE) as isize)
                    as u64)
                    .wrapping_mul(self.nanos_per_tick);
                let nice = (*cpu_info
                    .offset((cpu as i32 * libc::CPU_STATE_MAX + libc::CPU_STATE_NICE) as isize)
                    as u64)
                    .wrapping_mul(self.nanos_per_tick);
                let busy = user.wrapping_add(system.wrapping_add(nice));

                self.counters_percpu[cpu as usize][0].set(user);
                self.counters_percpu[cpu as usize][1].set(nice);
                self.counters_percpu[cpu as usize][2].set(system);
                self.counters_percpu[cpu as usize][3].set(idle);
                self.counters_percpu[cpu as usize][4].set(busy);

                total_user += user;
                total_system += system;
                total_idle += idle;
                total_nice += nice;
                total_busy += busy;
            }

            self.counters_total[0].set(elapsed, total_user);
            self.counters_total[1].set(elapsed, total_nice);
            self.counters_total[2].set(elapsed, total_system);
            self.counters_total[3].set(elapsed, total_idle);
            self.counters_total[4].set(elapsed, total_busy);
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to refresh processor info",
            ));
        }

        Ok(())
    }
}
