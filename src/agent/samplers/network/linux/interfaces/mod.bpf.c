// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

#include <vmlinux.h>
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>

#define COUNTER_GROUP_WIDTH 8
#define MAX_CPUS 1024

#define TX 0
#define TX_DROPPED 1

// counters
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");


SEC("tracepoint/net/net_dev_xmit")
int net_dev_xmit(struct trace_event_raw_net_dev_xmit *args)
{
	u32 offset = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();

	u32 idx = 0;

	if (args->rc != 0) {
		idx = offset + TX_DROPPED;

		array_incr(&counters, idx);
	}

	idx = offset + TX;

	array_incr(&counters, idx);

	return 0;
}

char LICENSE[] SEC("license") = "GPL";
