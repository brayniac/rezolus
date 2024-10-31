pub mod counter;
pub mod group;

pub use counter::Counter;

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
	groups: Vec<Option<PerfGroup>>,
}

impl PerfGroups {
	pub fn new() -> Self {
		let cpus = common::linux::cpus().expect("failed to get inventory of CPUs");

		let mut groups = Vec::with_capacity(cpus.len());

		for cpu in cpus {
			match PerfGroup::new(cpu) {
                Ok(g) => {
                	groups.push(Some(g));
                }
                Err(_) => {
                    warn!("Failed to create the perf group on CPU {}", cpu);
                    groups.push(None);
                }
            };
		}

		Self {
			groups,
		}
	}
}
