pub mod counter;
pub mod group;

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
	pub fn new() -> Result<Self, std::io::Error> {
		let cpus = common::linux::cpus()?;

		let mut groups = Vec::with_capacity(cpus.len());

		let mut empty = true;

		for cpu in cpus {
			match PerfGroup::new(cpu) {
                Ok(g) => {
                	empty = fasle;
                	groups.push(Some(g));
                }
                Err(_) => {
                    warn!("Failed to create the perf group on CPU {}", cpu);
                    groups.push(None);
                }
            };
		}

		if empty {
			Err(std::io::Error::other(
                "Failed to create perf group on any CPU",
            ))
		} else {
			Ok(Self {
				groups,
			})
		}
	}
}
