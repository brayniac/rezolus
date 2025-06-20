// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2020 Anton Protopopov
// Copyright (c) 2023 The Rezolus Authors
//
// Based on syscount(8) from BCC by Sasha Goldshtein

// NOTICE: this file is based off `syscount.bpf.c` from the BCC project
// <https://github.com/iovisor/bcc/> and has been modified for use within
// Rezolus.

// This BPF program tracks syscall enter and exit to provide metrics about
// syscall counts and latencies.

#include <vmlinux.h>
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>

#define COUNTER_GROUP_WIDTH 16
#define HISTOGRAM_BUCKETS HISTOGRAM_BUCKETS_POW_3
#define HISTOGRAM_POWER 3
#define MAX_CPUS 1024
#define MAX_CGROUPS 4096
#define MAX_PID 4194304
#define MAX_SYSCALL_ID 1024
#define RINGBUF_CAPACITY 262144

// dummy instance for skeleton to generate definition
struct cgroup_info _cgroup_info = {};

// provides a lookup table from syscall id to a counter index offset
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_SYSCALL_ID);
} syscall_lut SEC(".maps");

/*
 * tracking structures
 */

// ringbuf to pass cgroup info
struct {
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(key_size, 0);
	__uint(value_size, 0);
	__uint(max_entries, RINGBUF_CAPACITY);
} cgroup_info SEC(".maps");

// holds known cgroup serial numbers to help determine new or changed groups
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_serial_numbers SEC(".maps");

// hold the syscall start time per-pid to calculate latency
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, MAX_PID);
	__type(key, u32);
	__type(value, u64);
} start SEC(".maps");


/*
 * system-wide counters
 */

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");


/*
 * per-cgroup counters
 */

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_other SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_read SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_write SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_poll SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_lock SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_time SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_sleep SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_socket SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_yield SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_filesystem SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_memory SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_process SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_query SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_ipc SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_timer SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_syscall_event SEC(".maps");

/*
 * latency histograms
 */

// tracks the latency distribution of all other syscalls
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} other_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} read_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} write_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} poll_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} lock_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} time_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} sleep_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} socket_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} yield_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} filesystem_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} memory_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} process_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} query_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} ipc_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} timer_latency SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS);
} event_latency SEC(".maps");

SEC("tracepoint/raw_syscalls/sys_enter")
int sys_enter(struct trace_event_raw_sys_enter *args)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 tid = id;
	u64 ts;

	ts = bpf_ktime_get_ns();
	bpf_map_update_elem(&start, &tid, &ts, 0);

	u32 offset, idx, group = 0;
	u64 *elem;

	if (args->id < 0) {
		return 0;
	}

	u32 syscall_id = args->id;
	offset = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();

	// for some syscalls, we track counts by "family" of syscall. check the
	// lookup table and increment the appropriate counter
	idx = 0;
	if (syscall_id < MAX_SYSCALL_ID) {
		u32 *counter_offset = bpf_map_lookup_elem(&syscall_lut, &syscall_id);

		if (counter_offset && *counter_offset && *counter_offset < COUNTER_GROUP_WIDTH) {
			group = (u32)*counter_offset;
		}
	}

	idx = offset + group;
	array_incr(&counters, idx);

	struct task_struct *current = bpf_get_current_task_btf();

	if (bpf_core_field_exists(current->sched_task_group)) {
		int cgroup_id = current->sched_task_group->css.id;
		u64	serial_nr = current->sched_task_group->css.serial_nr;

		if (cgroup_id && cgroup_id < MAX_CGROUPS) {

			// we check to see if this is a new cgroup by checking the serial number

			elem = bpf_map_lookup_elem(&cgroup_serial_numbers, &cgroup_id);

			if (elem && *elem != serial_nr) {
				// zero the counters, they will not be exported until they are non-zero
				u64 zero = 0;
				bpf_map_update_elem(&cgroup_syscall_other, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_read, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_write, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_poll, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_lock, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_time, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_sleep, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_socket, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_yield, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_filesystem, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_memory, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_process, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_query, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_ipc, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_timer, &cgroup_id, &zero, BPF_ANY);
				bpf_map_update_elem(&cgroup_syscall_event, &cgroup_id, &zero, BPF_ANY);

				// initialize the cgroup info
				struct cgroup_info cginfo = {
					.id = cgroup_id,
					.level = current->sched_task_group->css.cgroup->level,
				};

				// read the cgroup name
				bpf_probe_read_kernel_str(&cginfo.name, CGROUP_NAME_LEN, current->sched_task_group->css.cgroup->kn->name);

				// read the cgroup parent name
				bpf_probe_read_kernel_str(&cginfo.pname, CGROUP_NAME_LEN, current->sched_task_group->css.cgroup->kn->parent->name);

				// read the cgroup grandparent name
				bpf_probe_read_kernel_str(&cginfo.gpname, CGROUP_NAME_LEN, current->sched_task_group->css.cgroup->kn->parent->parent->name);

				// push the cgroup info into the ringbuf
				bpf_ringbuf_output(&cgroup_info, &cginfo, sizeof(cginfo), 0);

				// update the serial number in the local map
				bpf_map_update_elem(&cgroup_serial_numbers, &cgroup_id, &serial_nr, BPF_ANY);
			}

			switch (group) {
				case 1:
					array_incr(&cgroup_syscall_read, cgroup_id);
					break;
				case 2:
					array_incr(&cgroup_syscall_write, cgroup_id);
					break;
				case 3:
					array_incr(&cgroup_syscall_poll, cgroup_id);
					break;
				case 4:
					array_incr(&cgroup_syscall_lock, cgroup_id);
					break;
				case 5:
					array_incr(&cgroup_syscall_time, cgroup_id);
					break;
				case 6:
					array_incr(&cgroup_syscall_sleep, cgroup_id);
					break;
				case 7:
					array_incr(&cgroup_syscall_socket, cgroup_id);
					break;
				case 8:
					array_incr(&cgroup_syscall_yield, cgroup_id);
					break;
				case 9:
					array_incr(&cgroup_syscall_filesystem, cgroup_id);
					break;
				case 10:
					array_incr(&cgroup_syscall_memory, cgroup_id);
					break;
				case 11:
					array_incr(&cgroup_syscall_process, cgroup_id);
					break;
				case 12:
					array_incr(&cgroup_syscall_query, cgroup_id);
					break;
				case 13:
					array_incr(&cgroup_syscall_ipc, cgroup_id);
					break;
				case 14:
					array_incr(&cgroup_syscall_timer, cgroup_id);
					break;
				case 15:
					array_incr(&cgroup_syscall_event, cgroup_id);
					break;
				default:
					array_incr(&cgroup_syscall_other, cgroup_id);
					break;
			}
		}
	}

	return 0;
}

SEC("tracepoint/raw_syscalls/sys_exit")
int sys_exit(struct trace_event_raw_sys_exit *args)
{
	u64 id = bpf_get_current_pid_tgid();
	u64 *start_ts, lat = 0;
	u32 tid = id, group = 0;

	u32 idx;

	if (args->id < 0) {
		return 0;
	}

	u32 syscall_id = args->id;

	// lookup the start time
	start_ts = bpf_map_lookup_elem(&start, &tid);

	// possible we missed the start
	if (!start_ts || *start_ts == 0) {
		return 0;
	}

	// calculate the latency
	lat = bpf_ktime_get_ns() - *start_ts;

	// clear the start timestamp
	*start_ts = 0;

	// calculate the histogram index for this latency value
	idx = value_to_index(lat, HISTOGRAM_POWER);

	// increment latency histogram for the syscall family
	if (syscall_id < MAX_SYSCALL_ID) {
		u32 *counter_offset = bpf_map_lookup_elem(&syscall_lut, &syscall_id);

		if (counter_offset && *counter_offset && *counter_offset < COUNTER_GROUP_WIDTH) {
			group = (u32)*counter_offset;
		}
	}

	switch (group) {
		case 1:
			array_incr(&read_latency, idx);
			break;
		case 2:
			array_incr(&write_latency, idx);
			break;
		case 3:
			array_incr(&poll_latency, idx);
			break;
		case 4:
			array_incr(&lock_latency, idx);
			break;
		case 5:
			array_incr(&time_latency, idx);
			break;
		case 6:
			array_incr(&sleep_latency, idx);
			break;
		case 7:
			array_incr(&socket_latency, idx);
			break;
		case 8:
			array_incr(&yield_latency, idx);
			break;
		case 9:
			array_incr(&filesystem_latency, idx);
			break;
		case 10:
			array_incr(&memory_latency, idx);
			break;
		case 11:
			array_incr(&process_latency, idx);
			break;
		case 12:
			array_incr(&query_latency, idx);
			break;
		case 13:
			array_incr(&ipc_latency, idx);
			break;
		case 14:
			array_incr(&timer_latency, idx);
			break;
		case 15:
			array_incr(&event_latency, idx);
			break;
		default:
			array_incr(&other_latency, idx);
			break;
	}

	return 0;
}

char LICENSE[] SEC("license") = "GPL";
