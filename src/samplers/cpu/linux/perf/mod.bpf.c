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
#include "../../../common/bpf/histogram.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>

#define COUNTER_GROUP_WIDTH 8
#define MAX_CPUS 1024
#define MAX_CGROUP 4194304

#define TASK_RUNNING 0

// perf counters
#define CYCLES 0
#define INSTRUCTIONS 1
#define TSC 2
#define APERF 3
#define MPERF 4

/**
 * commit 2f064a59a1 ("sched: Change task_struct::state") changes
 * the name of task_struct::state to task_struct::__state
 * see:
 *     https://github.com/torvalds/linux/commit/2f064a59a1
 */
struct task_struct___o {
	volatile long int state;
} __attribute__((preserve_access_index));

struct task_struct___x {
	unsigned int __state;
} __attribute__((preserve_access_index));

static __always_inline __s64 get_task_state(void *task)
{
	struct task_struct___x *t = task;

	if (bpf_core_field_exists(t->__state))
		return BPF_CORE_READ(t, __state);
	return BPF_CORE_READ((struct task_struct___o *)task, state);
}

// perf counters by cgroup
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUP * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");

// previous values
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} perf_counters SEC(".maps");

// perf event maps. one per counter type

struct {
	__uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
	__uint(key_size, sizeof(u32));
	__uint(value_size, sizeof(u32));
} cycles SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
	__uint(key_size, sizeof(u32));
	__uint(value_size, sizeof(u32));
} instructions SEC(".maps");

SEC("tp_btf/sched_switch")
int handle__sched_switch(u64 *ctx)
{
	/* TP_PROTO(bool preempt, struct task_struct *prev,
	 *      struct task_struct *next)
	 */
	struct task_struct *prev = (struct task_struct *)ctx[1];
	struct task_struct *next = (struct task_struct *)ctx[2];

// 	u32 pid, idx;
// 	u64 *tsp, delta_ns, *cnt, offcpu_ns;

	u32 idx;
	u64 *cnt, c, i;

	u32 processor_id = bpf_get_smp_processor_id();
// 	u64 ts = bpf_ktime_get_ns();

// 	// prev task is moving from running
// 	// - read perf counters
// 	// - lookup previous values
// 	// - update cgroup counters
	if (get_task_state(prev) == TASK_RUNNING) {
		c = bpf_perf_event_read(&cycles, processor_id);
		i = bpf_perf_event_read(&instructions, processor_id);

		idx = COUNTER_GROUP_WIDTH * processor_id + CYCLES;
		cnt = bpf_map_lookup_elem(&perf_counters, &idx);

		if (cnt) {
			c = c - *cnt;


		}
	}


// 		// count involuntary context switch
// 		idx = COUNTER_GROUP_WIDTH * processor_id + IVCSW;
// 		cnt = bpf_map_lookup_elem(&counters, &idx);

// 		if (cnt) {
// 			__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
// 		}

// 		pid = prev->pid;

// 		// mark when it was enqueued
// 		bpf_map_update_elem(&enqueued_at, &pid, &ts, 0);

// 		// calculate how long it was running and increment stats
// 		tsp = bpf_map_lookup_elem(&running_at, &pid);
// 		if (tsp && *tsp) {
// 			delta_ns = ts - *tsp;

// 			// update histogram
// 			idx = value_to_index(delta_ns, HISTOGRAM_POWER);
// 			cnt = bpf_map_lookup_elem(&running, &idx);
// 			if (cnt) {
// 				__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
// 			}

// 			*tsp = 0;
// 		}
// 	}

// 	// for all tasks: track when it went off-cpu
// 	pid = prev->pid;

// 	// mark off-cpu at
// 	bpf_map_update_elem(&offcpu_at, &pid, &ts, 0);
	
// 	// next task has moved into running
// 	// - update next->pid running_at with now
// 	// - calculate how long next task was enqueued, update hist
// 	pid = next->pid;

// 	// update running_at
// 	bpf_map_update_elem(&running_at, &pid, &ts, 0);

// 	// calculate how long it was enqueued and increment stats
// 	tsp = bpf_map_lookup_elem(&enqueued_at, &pid);
// 	if (tsp && *tsp) {
// 		delta_ns = ts - *tsp;

// 		// update the histogram
// 		idx = value_to_index(delta_ns, HISTOGRAM_POWER);
// 		cnt = bpf_map_lookup_elem(&runqlat, &idx);
// 		if (cnt) {
// 			__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
// 		}

// 		*tsp = 0;

// 		// calculate how long it was off-cpu, not including runqueue wait,
// 		// and increment stats
// 		tsp = bpf_map_lookup_elem(&offcpu_at, &pid);
// 		if (tsp && *tsp) {
// 			offcpu_ns = ts - *tsp;

// 			if (offcpu_ns > delta_ns) {
// 				offcpu_ns = offcpu_ns - delta_ns;

// 				// update the histogram
// 				idx = value_to_index(offcpu_ns, HISTOGRAM_POWER);
// 				cnt = bpf_map_lookup_elem(&offcpu, &idx);
// 				if (cnt) {
// 					__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
// 				}
// 			}

// 			*tsp = 0;
// 		}
// 	}

	return 0;
}

char LICENSE[] SEC("license") = "GPL";