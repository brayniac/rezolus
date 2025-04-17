// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program tracks CPU throttling events in cgroups by monitoring
// scheduler events and collecting throttling metrics from cgroups

#include <vmlinux.h>
#include "../../../agent/bpf/cgroup_info.h"
#include "../../../agent/bpf/core_fixes.h"
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#define MAX_CPUS 1024
#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

// dummy instance for skeleton to generate definition
struct cgroup_info _cgroup_info = {};

// ringbuf to pass cgroup info
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(key_size, 0);
    __uint(value_size, 0);
    __uint(max_entries, RINGBUF_CAPACITY);
} cgroup_info SEC(".maps");

// cgroup throttled time
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_throttled_time SEC(".maps");

// cgroup throttled count
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_throttled_count SEC(".maps");

// Struct to hold throttling data for a cgroup
struct throttle_info {
    u64 nr_periods;
    u64 nr_throttled;
    u64 throttled_time;
};

// Map to hold the last throttling data point for each cgroup
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, MAX_CGROUPS);
    __type(key, u32);
    __type(value, struct throttle_info);
} last_throttle_data SEC(".maps");

// Helper to extract cgroup ID from task_struct
static u32 get_cgroup_id(struct task_struct *p) {
    struct cgroup *cgrp;
    
    // Get the task's cgroup
    cgrp = BPF_CORE_READ(p, cgroups, subsys[0], cgroup);
    if (!cgrp) {
        return 0;
    }
    
    // Get cgroup ID from kernfs_node
    return BPF_CORE_READ(cgrp, kn, id);
}

// Helper to update cgroup metadata
static void update_cgroup_info(struct cgroup *cgrp, u32 cgroup_id) {
    if (!cgrp || !cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return;
    }

    // Fill cgroup info
    struct cgroup_info cginfo = {
        .id = cgroup_id,
        .level = BPF_CORE_READ(cgrp, level),
    };
    
    // Read cgroup names
    bpf_probe_read_kernel_str(&cginfo.name, CGROUP_NAME_LEN, BPF_CORE_READ(cgrp, kn, name));
    
    // For parent and grandparent, check if they exist first
    struct kernfs_node *parent = BPF_CORE_READ(cgrp, kn, parent);
    if (parent) {
        bpf_probe_read_kernel_str(&cginfo.pname, CGROUP_NAME_LEN, BPF_CORE_READ(parent, name));
        
        struct kernfs_node *gparent = BPF_CORE_READ(parent, parent);
        if (gparent) {
            bpf_probe_read_kernel_str(&cginfo.gpname, CGROUP_NAME_LEN, BPF_CORE_READ(gparent, name));
        }
    }
    
    // Send cgroup info through ringbuf
    bpf_ringbuf_output(&cgroup_info, &cginfo, sizeof(cginfo), 0);
}

// We use the scheduler tracepoint to sample tasks and check their cgroups
SEC("tracepoint/sched/sched_switch")
int handle_sched_switch(struct trace_event_raw_sched_switch *ctx)
{
    struct task_struct *prev, *next;
    u32 prev_pid, next_pid;
    
    prev_pid = ctx->prev_pid;
    next_pid = ctx->next_pid;
    
    // Get task_struct for the previous and next tasks
    prev = (struct task_struct *)bpf_get_current_task();
    
    // Get cgroup ID for the previous task
    u32 cgroup_id = get_cgroup_id(prev);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return 0;
    }
    
    // Get cgroup
    struct cgroup *cgrp = BPF_CORE_READ(prev, cgroups, subsys[0], cgroup);
    if (!cgrp) {
        return 0;
    }
    
    // Update cgroup info if needed
    update_cgroup_info(cgrp, cgroup_id);
    
    return 0;
}

// Periodic task to update throttling statistics
SEC("perf_event")
int update_throttle_stats(struct bpf_perf_event_data *ctx)
{
    // A periodic task that's triggered by perf events
    // This will update throttling statistics from the known cgroups

    // This is a sampling task, so no specific logic required here
    // The userspace program will read the cgroup stats directly
    
    return 0;
}

char LICENSE[] SEC("license") = "GPL";