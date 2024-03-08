use super::*;

struct GroupData {
    inner: perf_event::GroupData,
}

impl core::ops::Deref for GroupData {
    type Target = perf_event::GroupData;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl GroupData {
    pub fn enabled_since(&self, prev: &Self) -> Option<std::time::Duration> {
        if let (Some(this), Some(prev)) = (self.time_enabled(), prev.time_enabled()) {
            Some(this - prev)
        } else {
            None
        }
    }

    pub fn running_since(&self, prev: &Self) -> Option<std::time::Duration> {
        if let (Some(this), Some(prev)) = (self.time_running(), prev.time_running()) {
            Some(this - prev)
        } else {
            None
        }
    }

    pub fn delta(&self, prev: &Self, counter: &perf_event::Counter) -> Option<u64> {
        if let (Some(this), Some(prev)) = (self.get(counter), prev.get(counter)) {
            Some(this.value() - prev.value())
        } else {
            None
        }
    }
}

enum PerfEvent {
    Cycles,
    Instructions,
    Tsc,
    Aperf,
    Mperf,
}

impl PerfEvent {
    fn event(&self) -> dyn Event {
        match self {
            Self::Cycles => Hardware::CYCLES,
            Self::Instructions => Hardware::INSTRUCTIONS,
            Self::Tsc => Msr::TSC,
            Self::Aperf => Msr::APERF,
            Self::Mperf => Msr::MPERF,
        }
    }
}

struct PerfCounters {
    cpu: usize,
    leader: PerfEvent,
    counters: Vec<Option<perf_event::Counter>>,
}

impl PerfCounters {
    pub fn new(cpu: usize, leader: PerfEvent) -> Result<Self, ()> {
        let mut counters = Vec::new();

        let counter = Builder::new(leader.event())
            .one_cpu(cpu)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .pinned(true)
            .read_format(
                ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING | ReadFormat::GROUP,
            )
            .build()
            .map_err(|_| ())?;

        counters.resize_with(leader as usize, None);
        counters.push(counter);

        Self { leader, counters }
    }

    pub fn add(&mut self, event: PerfEvent) -> Result<(), ()> {
        let counter = Builder::new(Hardware::INSTRUCTIONS)
            .one_cpu(self.cpu)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .build_with_group(&mut self.counters[self.leader])
            .map_err(|_| ())?;

        counters.resize_with(event as usize, None);
        counters.push(counter);
        Ok(())
    }

    pub fn enable(&mut self) -> Result<(), ()> {
        self.counters[self.leader as usize]
            .enable_group()
            .map_err(|e| {
                error!("failed to enable the perf group on CPU{}: {e}", self.cpu);
            })
    }

    pub fn read(&mut self) -> Result<GroupData, ()> {
        self.counters[self.leader as usize]
            .read_group()
            .map_err(|e| {
                warn!("failed to read the perf group on CPU{id}: {e}");
            })
            .map(|inner| GroupData { inner })
    }

    pub fn counter(&self, event: PerfEvent) -> Option<perf_event::Counter> {
        self.counters.get(event as usize)
    }

    pub fn delta(&self, prev: GroupData, curr: GroupData, event: PerfEvent) -> Option<u64> {
        let counter = self.counter(event)?;
        curr.delta(prev, counter).ok()
    }
}

pub struct Reading {
    /// The CPU this reading is from
    pub id: usize,
    pub cycles: Option<u64>,
    pub instructions: Option<u64>,
    pub ipkc: Option<u64>,
    pub ipus: Option<u64>,
    pub base_frequency_mhz: Option<u64>,
    pub running_frequency_mhz: Option<u64>,
}

/// Per-cpu perf event group that measure all tasks on one CPU
pub struct PerfGroup {
    /// The CPU this group measures
    id: usize,
    /// The perf counters
    counters: PerfCounters,
    /// prev holds the previous readings
    prev: Option<GroupData>,
}

impl PerfGroup {
    /// Create and enable the group on the cpu
    pub fn new(id: usize) -> Result<Self, ()> {
        let counters = PerfCounters::new(PerfCounter::Cycles)
            .unwrap_or_else(PerfCounters::new(PerfCounter::Tsc))?;

        if counters.leader == PerfEvent::Cycles {
            counters.add(PerfEvent::Instructions)?;
        }

        counters.add(PerfEvent::Tsc)?;
        counters.add(PerfEvent::Mperf)?;
        counters.add(PerfEvent::Aperf)?;

        counters.enable()?;

        let prev = counters.read().ok();

        return Ok(Self { id, counters, prev });
    }

    pub fn get_metrics(&mut self) -> Result<Reading, ()> {
        let current = self.counters.read();

        if self.prev.is_none() {
            self.prev = Some(current);
            return Err(());
        }

        let prev = self.prev.as_ref().unwrap();

        // When the CPU is offline, this.len() becomes 1
        if current.len() == 1 || current.len() != prev.len() {
            self.prev = Some(current);
            return Err(());
        }

        let enabled_us = current
            .enabled_since(prev)
            .ok_or(())
            .map(|v| v.as_micros() as u64)?;
        let running_us = current
            .running_since(prev)
            .ok_or(())
            .map(|v| v.as_micros() as u64)?;

        if running_us != enabled_us || running_us = 0 {
            self.prev = Some(current);
            return Err(());
        }

        let cycles = self.counters.delta(prev, curr, PerfEvent::Cycles);
        let instructions = self.counters.delta(prev, curr, PerfEvent::Instructions);
        let tsc = self.counters.delta(prev, curr, PerfEvent::Tsc);
        let aperf = self.counters.delta(prev, curr, PerfEvent::Aperf);
        let mperf = self.counters.delta(prev, curr, PerfEvent::Mperf);

        // computed metrics

        let ipkc = match (cycles, instructions) {
            (Some(cycles), Some(instructions)) => {
                if cycles > 0 {
                    Some(instructions * 1000 / cycles)
                } else {
                    None
                }
            }
            _ => None,
        };

        let base_frequency_mhz = if let Some(tsc) = tsc {
            Some(tsc / running_us)
        } else {
            None
        };

        let running_frequency_mhz = match (base_frequency_mhz, aperf, mperf) {
            (Some(base_frequency_mhz), Some(aperf), Some(mperf)) => {
                if mperf > 0 {
                    Some((base_frequency_mhz * aperf) / mperf)
                } else {
                    None
                }
            }
            _ => None,
        };

        let ipus = match (ipkc, aperf, mperf) {
            (Some(ipkc), Some(aperf), Some(mperf)) => {
                if mperf > 0 {
                    Some((ipkc * aperf) / mperf)
                } else {
                    None
                }
            }
            _ => None,
        };

        self.prev = Some(current);

        Ok(Reading {
            id: self.id,
            cycles,
            instructions,
            ipkc,
            ipus,
            base_frequency_mhz,
            running_frequency_mhz,
        })
    }
}
