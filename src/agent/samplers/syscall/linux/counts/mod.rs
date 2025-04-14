//! Collects Syscall stats using BPF and traces:
//! * `raw_syscalls/sys_enter`
//!
//! And produces these stats:
//! * `syscall`
//! * `cgroup_syscall`

const NAME: &str = "syscall_counts";

mod bpf {
    include!(concat!(env!("OUT_DIR"), "/syscall_counts.bpf.rs"));
}

mod stats;

use bpf::*;
use stats::*;

use super::syscall_lut;
use crate::agent::*;

use std::sync::Arc;

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
        for group in &[
            &CGROUP_SYSCALL_OTHER,
            &CGROUP_SYSCALL_READ,
            &CGROUP_SYSCALL_WRITE,
            &CGROUP_SYSCALL_POLL,
            &CGROUP_SYSCALL_LOCK,
            &CGROUP_SYSCALL_TIME,
            &CGROUP_SYSCALL_SLEEP,
            &CGROUP_SYSCALL_SOCKET,
            &CGROUP_SYSCALL_YIELD,
            &CGROUP_SYSCALL_FILESYSTEM,
            &CGROUP_SYSCALL_MEMORY,
            &CGROUP_SYSCALL_PROCESS,
            &CGROUP_SYSCALL_QUERY,
            &CGROUP_SYSCALL_IPC,
            &CGROUP_SYSCALL_TIMER,
            &CGROUP_SYSCALL_EVENT,
        ] {
            group.insert_metadata(id, "name".to_string(), name.clone());
        }
    }
}

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    set_name(1, "/".to_string());

    let counters = vec![
        &SYSCALL_OTHER,
        &SYSCALL_READ,
        &SYSCALL_WRITE,
        &SYSCALL_POLL,
        &SYSCALL_LOCK,
        &SYSCALL_TIME,
        &SYSCALL_SLEEP,
        &SYSCALL_SOCKET,
        &SYSCALL_YIELD,
        &SYSCALL_FILESYSTEM,
        &SYSCALL_MEMORY,
        &SYSCALL_PROCESS,
        &SYSCALL_QUERY,
        &SYSCALL_IPC,
        &SYSCALL_TIMER,
        &SYSCALL_EVENT,
    ];

    let bpf = BpfBuilder::new(ModSkelBuilder::default)
        .counters("counters", counters)
        .map("syscall_lut", syscall_lut())
        .packed_counters("cgroup_syscall_other", &CGROUP_SYSCALL_OTHER)
        .packed_counters("cgroup_syscall_read", &CGROUP_SYSCALL_READ)
        .packed_counters("cgroup_syscall_write", &CGROUP_SYSCALL_WRITE)
        .packed_counters("cgroup_syscall_poll", &CGROUP_SYSCALL_POLL)
        .packed_counters("cgroup_syscall_lock", &CGROUP_SYSCALL_LOCK)
        .packed_counters("cgroup_syscall_time", &CGROUP_SYSCALL_TIME)
        .packed_counters("cgroup_syscall_sleep", &CGROUP_SYSCALL_SLEEP)
        .packed_counters("cgroup_syscall_socket", &CGROUP_SYSCALL_SOCKET)
        .packed_counters("cgroup_syscall_yield", &CGROUP_SYSCALL_YIELD)
        .packed_counters("cgroup_syscall_filesystem", &CGROUP_SYSCALL_FILESYSTEM)
        .packed_counters("cgroup_syscall_memory", &CGROUP_SYSCALL_MEMORY)
        .packed_counters("cgroup_syscall_process", &CGROUP_SYSCALL_PROCESS)
        .packed_counters("cgroup_syscall_query", &CGROUP_SYSCALL_QUERY)
        .packed_counters("cgroup_syscall_ipc", &CGROUP_SYSCALL_IPC)
        .packed_counters("cgroup_syscall_timer", &CGROUP_SYSCALL_TIMER)
        .packed_counters("cgroup_syscall_event", &CGROUP_SYSCALL_EVENT)
        .ringbuf_handler("cgroup_info", handle_event)
        .build()?;

    Ok(Some(Box::new(bpf)))
}

impl SkelExt for ModSkel<'_> {
    fn map(&self, name: &str) -> &libbpf_rs::Map {
        match name {
            "cgroup_info" => &self.maps.cgroup_info,
            "cgroup_syscall_other" => &self.maps.cgroup_syscall_other,
            "cgroup_syscall_read" => &self.maps.cgroup_syscall_read,
            "cgroup_syscall_write" => &self.maps.cgroup_syscall_write,
            "cgroup_syscall_poll" => &self.maps.cgroup_syscall_poll,
            "cgroup_syscall_lock" => &self.maps.cgroup_syscall_lock,
            "cgroup_syscall_time" => &self.maps.cgroup_syscall_time,
            "cgroup_syscall_sleep" => &self.maps.cgroup_syscall_sleep,
            "cgroup_syscall_socket" => &self.maps.cgroup_syscall_socket,
            "cgroup_syscall_yield" => &self.maps.cgroup_syscall_yield,
            "cgroup_syscall_filesystem" => &self.maps.cgroup_syscall_filesystem,
            "cgroup_syscall_memory" => &self.maps.cgroup_syscall_memory,
            "cgroup_syscall_process" => &self.maps.cgroup_syscall_process,
            "cgroup_syscall_query" => &self.maps.cgroup_syscall_query,
            "cgroup_syscall_ipc" => &self.maps.cgroup_syscall_ipc,
            "cgroup_syscall_timer" => &self.maps.cgroup_syscall_timer,
            "cgroup_syscall_event" => &self.maps.cgroup_syscall_event,
            "counters" => &self.maps.counters,
            "syscall_lut" => &self.maps.syscall_lut,
            _ => unimplemented!(),
        }
    }
}

impl OpenSkelExt for ModSkel<'_> {
    fn log_prog_instructions(&self) {
        debug!(
            "{NAME} sys_enter() BPF instruction count: {}",
            self.progs.sys_enter.insn_cnt()
        );
    }
}
