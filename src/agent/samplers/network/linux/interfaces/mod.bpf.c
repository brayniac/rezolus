// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

#include <vmlinux.h>
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>

#define COUNTER_GROUP_WIDTH 8

// counters
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");


SEC("tracepoint/net/net_dev_xmit")
int net_dev_xmit(struct trace_event_net_dev_xmit *args)
{
	return 0;
}

char LICENSE[] SEC("license") = "GPL";
