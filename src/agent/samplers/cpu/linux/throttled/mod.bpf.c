// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program tracks CPU throttling events in cgroups

#include <vmlinux.h>
#include "../../../agent/bpf/cgroup_info.h"
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#define MAX_CPUS 1024
#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

// Define the tracepoint structure for cgroup tracepoints
struct cgroup_throttle_args {
    __u64 pad;
    __u64 id;           // cgroup id
    char  *path;        // cgroup path
    __u64 cpu_id;       // cpu id
    __u64 throttle_percent; // percentage throttled
    __u64 throttle_period_us; // throttle period in us
    __u64 quota_us;     // quota in us
    __u64 nr_periods;   // number of periods
    __u64 nr_throttled; // number of times throttled
};

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

// Helper to get cgroup details and update cgroup info
static void update_cgroup_info(u32 cgroup_id, u64 id) {
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS) {
        return;
    }

    struct task_struct *current = (struct task_struct *)bpf_get_current_task();
    void *task_group = BPF_CORE_READ(current, sched_task_group);
    if (task_group) {
        // Initialize cgroup info and counters
        u64 zero = 0;
        bpf_map_update_elem(&cgroup_throttled_time, &cgroup_id, &zero, BPF_ANY);
        bpf_map_update_elem(&cgroup_throttled_count, &cgroup_id, &zero, BPF_ANY);

        // Initialize the cgroup info
        struct cgroup_info cginfo = {
            .id = cgroup_id,
            .level = BPF_CORE_READ(current, sched_task_group, css.cgroup, level),
        };

        // read the cgroup name
        bpf_probe_read_kernel_str(&cginfo.name, CGROUP_NAME_LEN, BPF_CORE_READ(current, sched_task_group, css.cgroup, kn, name));

        // read the cgroup parent name
        bpf_probe_read_kernel_str(&cginfo.pname, CGROUP_NAME_LEN, BPF_CORE_READ(current, sched_task_group, css.cgroup, kn, parent, name));

        // read the cgroup grandparent name
        bpf_probe_read_kernel_str(&cginfo.gpname, CGROUP_NAME_LEN, BPF_CORE_READ(current, sched_task_group, css.cgroup, kn, parent, parent, name));

        // Push the cgroup info into the ringbuf
        bpf_ringbuf_output(&cgroup_info, &cginfo, sizeof(cginfo), 0);

        // Update the serial number in the local map
        bpf_map_update_elem(&cgroup_serial_numbers, &cgroup_id, &id, BPF_ANY);
    }
}

// Handler for throttling start
SEC("tracepoint/cgroup/cgroup_throttle_cpu")
int handle_throttle_start(struct cgroup_throttle_args *ctx)
{
    u32 cgroup_id = (u32)ctx->id;
    u64 ts = bpf_ktime_get_ns();
    u64 *elem;

    if (cgroup_id && cgroup_id < MAX_CGROUPS) {
        // Check if this is a new cgroup
        elem = bpf_map_lookup_elem(&cgroup_serial_numbers, &cgroup_id);
        if (elem && *elem != ctx->id) {
            update_cgroup_info(cgroup_id, ctx->id);
        }

        // Record the throttling start time
        bpf_map_update_elem(&throttling_start, &cgroup_id, &ts, BPF_ANY);

        // Increment throttle count
        elem = bpf_map_lookup_elem(&cgroup_throttled_count, &cgroup_id);
        if (elem) {
            (*elem)++;
        }
    }

    return 0;
}

// Handler for throttling end
SEC("tracepoint/cgroup/cgroup_unthrottle_cpu")
int handle_throttle_end(struct cgroup_throttle_args *ctx)
{
    u32 cgroup_id = (u32)ctx->id;
    u64 ts, *start_ts, *throttled_time;
    u64 duration;

    if (cgroup_id && cgroup_id < MAX_CGROUPS) {
        // Get the throttling start time
        start_ts = bpf_map_lookup_elem(&throttling_start, &cgroup_id);
        if (!start_ts || *start_ts == 0) {
            return 0;
        }

        // Calculate throttling duration
        ts = bpf_ktime_get_ns();
        if (*start_ts > ts) {
            // Handle timestamp overflow case
            duration = 0;
        } else {
            duration = ts - *start_ts;
        }

        // Update the throttled time counter
        throttled_time = bpf_map_lookup_elem(&cgroup_throttled_time, &cgroup_id);
        if (throttled_time) {
            *throttled_time += duration;
        }

        // Reset start time to 0 instead of deleting (since we're using an array)
        u64 zero = 0;
        bpf_map_update_elem(&throttling_start, &cgroup_id, &zero, BPF_ANY);
    }

    return 0;
}

char LICENSE[] SEC("license") = "GPL";