//! Collects CPU Throttling stats using BPF and traces:
//! * `cgroup/cgroup_throttle_cpu`
//! * `cgroup/cgroup_unthrottle_cpu`
//!
//! And produces these stats:
//! * `cgroup_cpu_throttled_time`
//! * `cgroup_cpu_throttled_count`
//!
//! These stats can be used to understand when and for how long cgroups
//! are being throttled by the CPU controller.

const NAME: &str = "cpu_throttled";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/cpu_throttled.bpf.rs"));
}

use bpf::*;

use crate::agent::*;

use std::sync::Arc;

mod stats;

use stats::*;

unsafe impl plain::Plain for bpf::types::cgroup_info {}

fn handle_event(data: &[u8]) -> i32 {
    let mut cgroup_info = bpf::types::cgroup_info::default();

    if plain::copy_from_bytes(&mut cgroup_info, data).is_ok() {
        let name = std::str::from_utf8(&cgroup_info.name)
            .unwrap()
            .trim_end_matches(char::from(0))
            .replace("\\x2d", "-");

        let pname = std::str::from_utf8(&cgroup_info.pname)
            .unwrap()
            .trim_end_matches(char::from(0))
            .replace("\\x2d", "-");

        let gpname = std::str::from_utf8(&cgroup_info.gpname)
            .unwrap()
            .trim_end_matches(char::from(0))
            .replace("\\x2d", "-");

        let name = if !gpname.is_empty() {
            if cgroup_info.level > 3 {
                format!(".../{gpname}/{pname}/{name}")
            } else {
                format!("/{gpname}/{pname}/{name}")
            }
        } else if !pname.is_empty() {
            format!("/{pname}/{name}")
        } else if !name.is_empty() {
            format!("/{name}")
        } else {
            "".to_string()
        };

        let id = cgroup_info.id;

        set_name(id as usize, name)
    }

    0
}

fn set_name(id: usize, name: String) {
    if !name.is_empty() {
        CGROUP_CPU_THROTTLED_TIME.insert_metadata(id, "name".to_string(), name.clone());
        CGROUP_CPU_THROTTLED_COUNT.insert_metadata(id, "name".to_string(), name);
    }
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    // Set the root cgroup name
    set_name(1, "/".to_string());

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .packed_counters("cgroup_throttled_time", &CGROUP_CPU_THROTTLED_TIME)
        .packed_counters("cgroup_throttled_count", &CGROUP_CPU_THROTTLED_COUNT)
        .ringbuf_handler("cgroup_info", handle_event)
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_throttled_time" => &self.maps.cgroup_throttled_time,
            "cgroup_throttled_count" => &self.maps.cgroup_throttled_count,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} handle_throttle_start() BPF instruction count: {}",
            self.progs.handle_throttle_start.insn_cnt()
        );
        debug!(
            "{NAME} handle_throttle_end() BPF instruction count: {}",
            self.progs.handle_throttle_end.insn_cnt()
        );
    }
}