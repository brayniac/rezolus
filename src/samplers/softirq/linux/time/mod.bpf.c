// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program tracks irq handler enter and exit to provide metrics about
// interrupts.

#include <vmlinux.h>
#include "../../../common/bpf/irq_info.h"
#include "../../../common/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>

#define COUNTER_GROUP_WIDTH 8
#define MAX_CPUS 1024
#define MAX_IRQS 4096

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, MAX_CPUS);
	__type(key, u32);
	__type(value, u64);
} start SEC(".maps");

// counters for irq time in nanoseconds
// 0 - other
// 1..COUNTER_GROUP_WIDTH - grouped interrupts defined in userspace in the
//                          `irq_lut` map
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");

// provides a lookup table from syscall id to a counter index offset
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_IRQS);
} irq_lut SEC(".maps");

SEC("tracepoint/irq/softirq_entry")
int softirq_enter(struct trace_event_raw_softirq *args)
{
	u32 cpu = bpf_get_smp_processor_id();
	u64 ts = bpf_ktime_get_ns();

	bpf_map_update_elem(&start, &cpu, &ts, 0);

	return 0;
}

SEC("tracepoint/irq/softirq_exit")
int softirq_exit(struct trace_event_raw_softirq *args)
{
	u32 cpu = bpf_get_smp_processor_id();
	u64 *elem, *start_ts, dur = 0;
	u32 offset, idx, group = 0;

	u32 irq_id = 0;

	// lookup the start time
	start_ts = bpf_map_lookup_elem(&start, &cpu);

	// possible we missed the start
	if (!start_ts || *start_ts == 0) {
		return 0;
	}

	// calculate the duration
	dur = bpf_ktime_get_ns() - *start_ts;

	offset = COUNTER_GROUP_WIDTH * cpu;

	if (irq_id < MAX_IRQS) {
		u32 *counter_offset = bpf_map_lookup_elem(&irq_lut, &irq_id);

		if (counter_offset && *counter_offset && *counter_offset < COUNTER_GROUP_WIDTH) {
			group = (u32)*counter_offset;
		}
	}

	idx = offset + group;
	array_add(&counters, idx, dur);

	// clear the start timestamp
	*start_ts = 0;

	return 0;
}


char LICENSE[] SEC("license") = "GPL";
