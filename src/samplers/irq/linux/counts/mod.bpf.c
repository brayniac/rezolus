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

// counters for syscalls
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

SEC("tracepoint/irq/irq_handler_entry")
int irq_enter(struct trace_event_raw_irq_handler_entry *args)
{
	u32 offset, idx, group = 0;
	u64 *elem;

	u32 irq_id = args->irq;

	offset = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();

	if (irq_id < MAX_IRQS) {
		u32 *counter_offset = bpf_map_lookup_elem(&irq_lut, &irq_id);

		if (counter_offset && *counter_offset && *counter_offset < COUNTER_GROUP_WIDTH) {
			group = (u32)*counter_offset;
		}
	}

	idx = offset + group;
	array_incr(&counters, idx);

	return 0;
}

char LICENSE[] SEC("license") = "GPL";
