use crate::*;

use super::stats::*;
use crate::common::units::{KIBIBYTES, MICROSECONDS, SECONDS};

#[distributed_slice(SAMPLERS)]
fn init(config: &Config) -> Option<Box<dyn Sampler>> {
    if let Ok(rusage) = Rusage::new(config) {
        Some(Box::new(rusage))
    } else {
        None
    }
}

const NAME: &str = "rezolus_rusage";

pub struct Rusage {
    interval: Interval,
    ru_utime: CounterWithHist,
    ru_stime: CounterWithHist,
}

impl Rusage {
    pub fn new(config: &Config) -> Result<Self, ()> {
        // check if sampler should be enabled
        if !config.enabled(NAME) {
            return Err(());
        }

        Ok(Self {
            interval: config.interval(NAME),
            ru_utime: CounterWithHist::new(&RU_UTIME, &RU_UTIME_HISTOGRAM),
            ru_stime: CounterWithHist::new(&RU_STIME, &RU_STIME_HISTOGRAM),
        })
    }
}

#[async_trait]
impl Sampler for Rusage {
    async fn sample(&mut self) {
        let elapsed = self.interval.tick().await;

        let now = Instant::now();
        
        METADATA_REZOLUS_RUSAGE_COLLECTED_AT.set(UnixInstant::EPOCH.elapsed().as_nanos());
        
        self.sample_rusage(elapsed);

        let elapsed = now.elapsed().as_nanos() as u64;
        METADATA_REZOLUS_RUSAGE_RUNTIME.add(elapsed);
        let _ = METADATA_REZOLUS_RUSAGE_RUNTIME_HISTOGRAM.increment(elapsed);
    }

    fn is_fast(&self) -> bool {
        true
    }
}

impl Rusage {
    fn sample_rusage(&mut self, elapsed: Option<Duration>) {
        let mut rusage = libc::rusage {
            ru_utime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_stime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_maxrss: 0,
            ru_ixrss: 0,
            ru_idrss: 0,
            ru_isrss: 0,
            ru_minflt: 0,
            ru_majflt: 0,
            ru_nswap: 0,
            ru_inblock: 0,
            ru_oublock: 0,
            ru_msgsnd: 0,
            ru_msgrcv: 0,
            ru_nsignals: 0,
            ru_nvcsw: 0,
            ru_nivcsw: 0,
        };

        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage) } == 0 {
            self.ru_utime.set(
                elapsed,
                rusage.ru_utime.tv_sec as u64 * SECONDS
                    + rusage.ru_utime.tv_usec as u64 * MICROSECONDS,
            );
            self.ru_stime.set(
                elapsed,
                rusage.ru_stime.tv_sec as u64 * SECONDS
                    + rusage.ru_stime.tv_usec as u64 * MICROSECONDS,
            );
            RU_MAXRSS.set(rusage.ru_maxrss * KIBIBYTES as i64);
            RU_MINFLT.set(rusage.ru_minflt as u64);
            RU_MAJFLT.set(rusage.ru_majflt as u64);
            RU_INBLOCK.set(rusage.ru_inblock as u64);
            RU_OUBLOCK.set(rusage.ru_oublock as u64);
            RU_NVCSW.set(rusage.ru_nvcsw as u64);
            RU_NIVCSW.set(rusage.ru_nivcsw as u64);
        }
    }
}
