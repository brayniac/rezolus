// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2023 The Rezolus Authors

#include <vmlinux.h>
#include "../../../common/bpf/histogram.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>

#define COUNTER_GROUP_WIDTH 8
#define MAX_CPUS 1024

// counter positions
#define CYCLES 0
#define INSTRUCTIONS 1
#define TSC 2
#define APERF 3
#define MPERF 4

// counters (see constants defined at top)
struct {
	__uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u32);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");
