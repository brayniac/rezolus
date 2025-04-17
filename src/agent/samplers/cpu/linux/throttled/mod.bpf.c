// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program tracks CPU throttling events in cgroups by probing
// the kernel's CPU controller functions

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

// holds known cgroup serial numbers to help determine new or changed groups
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_serial_numbers SEC(".maps");

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

// track start times for throttling events
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} throttling_start SEC(".maps");

// Helper to get and update cgroup metadata
static void update_cgroup_metadata(struct cgroup *cgrp, u32 cgroup_id) {
    if (!cgrp || !cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return;
    }

    u64 serial_nr = BPF_CORE_READ(cgrp, kn, id);
    u64 *elem = bpf_map_lookup_elem(&cgroup_serial_numbers, &cgroup_id);

    // Check if this is a new cgroup or one we haven't seen before
    if (!elem || *elem != serial_nr) {
        // Initialize counters
        u64 zero = 0;
        bpf_map_update_elem(&cgroup_throttled_time, &cgroup_id, &zero, BPF_ANY);
        bpf_map_update_elem(&cgroup_throttled_count, &cgroup_id, &zero, BPF_ANY);

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

        // Update serial number
        bpf_map_update_elem(&cgroup_serial_numbers, &cgroup_id, &serial_nr, BPF_ANY);
    }
}

// Start throttling when CPU controller decides to throttle a cgroup
SEC("kprobe/cpu_cfs_throttle")
int BPF_KPROBE(cpu_cfs_throttle_enter, struct task_struct *p)
{
    struct cgroup *cgrp;

    // Get the task's cgroup
    cgrp = BPF_CORE_READ(p, cgroups, subsys[0], cgroup);
    if (!cgrp) {
        return 0;
    }

    u32 cgroup_id = BPF_CORE_READ(cgrp, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return 0;
    }

    // Update cgroup metadata if needed
    update_cgroup_metadata(cgrp, cgroup_id);

    // Record throttling start time
    u64 ts = bpf_ktime_get_ns();
    bpf_map_update_elem(&throttling_start, &cgroup_id, &ts, BPF_ANY);

    // Increment throttle count
    u64 *count = bpf_map_lookup_elem(&cgroup_throttled_count, &cgroup_id);
    if (count) {
        (*count)++;
    }

    return 0;
}

// End throttling when a CPU becomes unthrottled
SEC("kprobe/cpu_cfs_unthrottle")
int BPF_KPROBE(cpu_cfs_unthrottle_enter, struct task_struct *p)
{
    struct cgroup *cgrp;

    // Get the task's cgroup
    cgrp = BPF_CORE_READ(p, cgroups, subsys[0], cgroup);
    if (!cgrp) {
        return 0;
    }

    u32 cgroup_id = BPF_CORE_READ(cgrp, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return 0;
    }

    // Get throttling start time
    u64 *start_ts = bpf_map_lookup_elem(&throttling_start, &cgroup_id);
    if (!start_ts || *start_ts == 0) {
        return 0;
    }

    // Calculate throttling duration
    u64 ts = bpf_ktime_get_ns();
    u64 duration = 0;

    if (*start_ts <= ts) {
        duration = ts - *start_ts;
    }

    // Update throttled time
    u64 *throttled_time = bpf_map_lookup_elem(&cgroup_throttled_time, &cgroup_id);
    if (throttled_time && duration > 0) {
        *throttled_time += duration;
    }

    // Reset start time
    u64 zero = 0;
    bpf_map_update_elem(&throttling_start, &cgroup_id, &zero, BPF_ANY);

    return 0;
}

// Additional probe to catch throttling when looking at task's runnable status
SEC("kprobe/tg_throttle_up")
int BPF_KPROBE(tg_throttle_up_enter, struct task_group *tg, unsigned long *flags)
{
    struct cgroup *cgrp;

    // Get the task group's cgroup
    cgrp = BPF_CORE_READ(tg, css.cgroup);
    if (!cgrp) {
        return 0;
    }

    u32 cgroup_id = BPF_CORE_READ(cgrp, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return 0;
    }

    // Update cgroup metadata if needed
    update_cgroup_metadata(cgrp, cgroup_id);

    // Get throttling start time
    u64 *start_ts = bpf_map_lookup_elem(&throttling_start, &cgroup_id);
    if (!start_ts || *start_ts == 0) {
        return 0;
    }

    // Calculate throttling duration
    u64 ts = bpf_ktime_get_ns();
    u64 duration = 0;

    if (*start_ts <= ts) {
        duration = ts - *start_ts;
    }

    // Update throttled time
    u64 *throttled_time = bpf_map_lookup_elem(&cgroup_throttled_time, &cgroup_id);
    if (throttled_time && duration > 0) {
        *throttled_time += duration;
    }

    // Reset start time
    u64 zero = 0;
    bpf_map_update_elem(&throttling_start, &cgroup_id, &zero, BPF_ANY);

    return 0;
}

// Track throttling down
SEC("kprobe/tg_throttle_down")
int BPF_KPROBE(tg_throttle_down_enter, struct task_group *tg, unsigned long *flags)
{
    struct cgroup *cgrp;

    // Get the task group's cgroup
    cgrp = BPF_CORE_READ(tg, css.cgroup);
    if (!cgrp) {
        return 0;
    }

    u32 cgroup_id = BPF_CORE_READ(cgrp, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return 0;
    }

    // Update cgroup metadata if needed
    update_cgroup_metadata(cgrp, cgroup_id);

    // Record throttling start time
    u64 ts = bpf_ktime_get_ns();
    bpf_map_update_elem(&throttling_start, &cgroup_id, &ts, BPF_ANY);

    // Increment throttle count
    u64 *count = bpf_map_lookup_elem(&cgroup_throttled_count, &cgroup_id);
    if (count) {
        (*count)++;
    }

    return 0;
}

char LICENSE[] SEC("license") = "GPL";