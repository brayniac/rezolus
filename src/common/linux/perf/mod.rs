mod counter;
mod group;

pub use counter::Counter;
pub use group::Reading;

use crate::*;

use group::PerfGroup;

use crate::common;

use tokio::sync::Mutex;

use std::sync::LazyLock;

pub static PERF_GROUPS: LazyLock<Mutex<PerfGroups>> = LazyLock::new(|| {
    Mutex::new(PerfGroups::new())
});

/// Contains one `PerfGroup` per CPU.
pub struct PerfGroups {
	groups: Vec<PerfGroup>,
}

impl PerfGroups {
	pub fn new() -> Self {
		let cpus = common::linux::cpus().expect("failed to get inventory of CPUs");

		let mut groups = Vec::with_capacity(cpus.len());

		for cpu in cpus {
			match PerfGroup::new(cpu) {
                Ok(g) => {
                	groups.push(g);
                }
                Err(_) => {
                    warn!("Failed to create the perf group on CPU {}", cpu);
                }
            };
		}

		info!("PerfGroups created for {} out of {} cpus", groups.len(), cpus.len());

		Self {
			groups,
		}
	}

	pub fn readings(&mut self) -> Vec<Reading> {
		let mut result = Vec::new();

		for group in &mut self.groups {
			if let Ok(reading) = group.get_metrics() {
				result.push(reading);
			}
		}

		result
	}
}
