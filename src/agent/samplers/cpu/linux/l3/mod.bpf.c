// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2023 The Rezolus Authors

#include <vmlinux.h>
#include "../../../agent/bpf/cgroup_info.h"
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#define COUNTERS 2
#define COUNTER_GROUP_WIDTH 8
#define MAX_CPUS 1024
#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

/**
 * perf event arrays
 */

struct {
	__uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
	__type(key, u32);
	__type(value, u32);
} l3_access SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
	__type(key, u32);
	__type(value, u32);
} l3_miss SEC(".maps");

// attach a tracepoint on sched_switch for per-cgroup accounting

SEC("tp_btf/sched_switch")
int handle__sched_switch(u64 *ctx)
{
	return 0;
}

char LICENSE[] SEC("license") = "GPL";
