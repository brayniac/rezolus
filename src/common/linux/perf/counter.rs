use crate::*;

use perf_event::events::x86::{Msr, MsrId};
use perf_event::events::Hardware;
use perf_event::{Builder, ReadFormat};


#[derive(Copy, Clone, Debug)]
pub enum Counter {
    Cycles,
    Instructions,
    Tsc,
    Aperf,
    Mperf,
}

impl Counter {
    fn builder(&self) -> Result<perf_event::Builder, std::io::Error> {
        match self {
            Self::Cycles => Ok(Builder::new(Hardware::CPU_CYCLES)),
            Self::Instructions => Ok(Builder::new(Hardware::INSTRUCTIONS)),
            Self::Tsc => {
                let msr = Msr::new(MsrId::TSC)?;
                Ok(Builder::new(msr))
            }
            Self::Aperf => {
                let msr = Msr::new(MsrId::APERF)?;
                Ok(Builder::new(msr))
            }
            Self::Mperf => {
                let msr = Msr::new(MsrId::MPERF)?;
                Ok(Builder::new(msr))
            }
        }
    }

    pub fn as_leader(&self, cpu: usize) -> Result<perf_event::Counter, std::io::Error> {
        self.builder()?
            .one_cpu(cpu)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .pinned(true)
            .read_format(
                ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING | ReadFormat::GROUP,
            )
            .build()
    }

    pub fn as_follower(
        &self,
        cpu: usize,
        leader: &mut perf_event::Counter,
    ) -> Result<perf_event::Counter, std::io::Error> {
        self.builder()?
            .one_cpu(cpu)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .build_with_group(leader)
    }
}