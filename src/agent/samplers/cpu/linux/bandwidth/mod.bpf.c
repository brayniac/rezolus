// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program probes CFS bandwidth control events to gather detailed metrics

#include <vmlinux.h>
#include "../../../agent/bpf/cgroup_info.h"
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

// Custom structure to pass bandwidth info to userspace
struct bandwidth_info {
    u32 id;             // cgroup id
    u64 quota;          // quota in nanoseconds
    u64 period;         // period in nanoseconds
    u64 runtime;        // runtime in nanoseconds
};

// dummy instance for skeleton to generate definition
struct cgroup_info _cgroup_info = {};
struct bandwidth_info _bandwidth_info = {};

// ringbuf to pass cgroup info
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(key_size, 0);
    __uint(value_size, 0);
    __uint(max_entries, RINGBUF_CAPACITY);
} cgroup_info SEC(".maps");

// ringbuf to pass bandwidth info
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(key_size, 0);
    __uint(value_size, 0);
    __uint(max_entries, RINGBUF_CAPACITY);
} bandwidth_info SEC(".maps");

// holds known cgroup serial numbers to help determine new or changed groups
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_serial_numbers SEC(".maps");

// counters

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} quota_consumed SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} period_events SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} redistribution SEC(".maps");

// Throttling counters (moved from cpu_throttled module)
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} throttled_time SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} throttled_count SEC(".maps");

// Map to track cfs_bandwidth pointers to cgroup ids
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, MAX_CGROUPS);
    __type(key, void *);  // cfs_bandwidth pointer
    __type(value, u32);   // cgroup id
} cfs_b_to_cgroup SEC(".maps");

// Track the last time a cgroup consumed runtime
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, MAX_CGROUPS);
    __type(key, u32);
    __type(value, u64);
} last_runtime SEC(".maps");

SEC("kprobe/tg_set_cfs_bandwidth")
int tg_set_cfs_bandwidth(struct pt_regs *ctx)
{
    struct task_group *tg = (struct task_group *)PT_REGS_PARM1(ctx);
    struct cfs_bandwidth *cfs_b = (struct cfs_bandwidth *)PT_REGS_PARM2(ctx);

    if (!tg || !cfs_b)
        return 0;

    // Get cgroup information
    struct cgroup_subsys_state *css = &tg->css;
    if (!css)
        return 0;

    u32 cgroup_id = BPF_CORE_READ(css, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS)
        return 0;

    u64 serial_nr = BPF_CORE_READ(css, serial_nr);

    // Store the mapping between cfs_bandwidth pointer and cgroup_id
    bpf_map_update_elem(&cfs_b_to_cgroup, &cfs_b, &cgroup_id, BPF_ANY);

    // Check if this is a new cgroup by checking the serial number
    u64 *elem = bpf_map_lookup_elem(&cgroup_serial_numbers, &cgroup_id);

    if (elem && *elem != serial_nr) {
        // Zero the counters, they will not be exported until they are non-zero
        u64 zero = 0;
        bpf_map_update_elem(&quota_consumed, &cgroup_id, &zero, BPF_ANY);
        bpf_map_update_elem(&period_events, &cgroup_id, &zero, BPF_ANY);
        bpf_map_update_elem(&redistribution, &cgroup_id, &zero, BPF_ANY);
        bpf_map_update_elem(&throttled_time, &cgroup_id, &zero, BPF_ANY);
        bpf_map_update_elem(&throttled_count, &cgroup_id, &zero, BPF_ANY);

        // Initialize the cgroup info
        struct cgroup_info cginfo = {
            .id = cgroup_id,
            .level = BPF_CORE_READ(css, cgroup, level),
        };

        // Read the cgroup name hierarchy
        bpf_probe_read_kernel_str(&cginfo.name, CGROUP_NAME_LEN, BPF_CORE_READ(css, cgroup, kn, name));
        bpf_probe_read_kernel_str(&cginfo.pname, CGROUP_NAME_LEN, BPF_CORE_READ(css, cgroup, kn, parent, name));
        bpf_probe_read_kernel_str(&cginfo.gpname, CGROUP_NAME_LEN, BPF_CORE_READ(css, cgroup, kn, parent, parent, name));
        
        // Push the cgroup info into the ringbuf
        bpf_ringbuf_output(&cgroup_info, &cginfo, sizeof(cginfo), 0);
        
        // Update the serial number in the local map
        bpf_map_update_elem(&cgroup_serial_numbers, &cgroup_id, &serial_nr, BPF_ANY);
    }

    // Read the quota and period values
    u64 quota = BPF_CORE_READ(cfs_b, quota);
    u64 period = BPF_CORE_READ(cfs_b, period);

    // Create bandwidth info to send to userspace
    struct bandwidth_info bw_info = {
        .id = cgroup_id,
        .quota = quota,
        .period = period,
        .runtime = 0
    };

    // Send bandwidth info to userspace
    bpf_ringbuf_output(&bandwidth_info, &bw_info, sizeof(bw_info), 0);

    return 0;
}

SEC("kprobe/tg_unthrottle_up")
int tg_unthrottle_up(struct pt_regs *ctx)
{
    struct task_group *tg = (struct task_group *)PT_REGS_PARM1(ctx);
    
    if (!tg)
        return 0;

    // Get cgroup information
    struct cgroup_subsys_state *css = &tg->css;
    if (!css)
        return 0;

    u32 cgroup_id = BPF_CORE_READ(css, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS)
        return 0;

    // Increment redistribution counter
    array_incr(&redistribution, cgroup_id);

    return 0;
}

SEC("kprobe/update_cpu_runtime")
int update_cpu_runtime(struct pt_regs *ctx)
{
    struct cfs_bandwidth *cfs_b = (struct cfs_bandwidth *)PT_REGS_PARM1(ctx);
    u64 runtime = (u64)PT_REGS_PARM2(ctx);
    
    if (!cfs_b)
        return 0;

    // Look up the cgroup_id using our mapping table
    u32 *cgroup_id_ptr = bpf_map_lookup_elem(&cfs_b_to_cgroup, &cfs_b);
    if (!cgroup_id_ptr || *cgroup_id_ptr == 0 || *cgroup_id_ptr >= MAX_CGROUPS)
        return 0;
    
    u32 cgroup_id = *cgroup_id_ptr;

    // Get the last runtime value
    u64 *last = bpf_map_lookup_elem(&last_runtime, &cgroup_id);
    u64 prev_runtime = last ? *last : 0;

    // If this is a new runtime measurement higher than the previous one,
    // add the difference to the quota_consumed counter
    if (runtime > prev_runtime) {
        u64 consumed = runtime - prev_runtime;
        array_add(&quota_consumed, cgroup_id, consumed);
    }

    // Update the last runtime
    bpf_map_update_elem(&last_runtime, &cgroup_id, &runtime, BPF_ANY);

    return 0;
}

SEC("kprobe/cfs_period_timer_fn")
int cfs_period_timer_fn(struct pt_regs *ctx)
{
    // This function is called when a CFS period expires
    struct cfs_bandwidth *cfs_b = (struct cfs_bandwidth *)PT_REGS_PARM1(ctx);
    
    if (!cfs_b)
        return 0;

    // Look up the cgroup_id using our mapping table
    u32 *cgroup_id_ptr = bpf_map_lookup_elem(&cfs_b_to_cgroup, &cfs_b);
    if (!cgroup_id_ptr || *cgroup_id_ptr == 0 || *cgroup_id_ptr >= MAX_CGROUPS)
        return 0;
    
    u32 cgroup_id = *cgroup_id_ptr;

    // Increment period events counter
    array_incr(&period_events, cgroup_id);

    return 0;
}

// Add the throttling handlers from the original cpu_throttled module
SEC("kprobe/throttle_cfs_rq")
int throttle_cfs_rq(struct pt_regs *ctx)
{
    struct cfs_rq *cfs_rq = (struct cfs_rq *)PT_REGS_PARM1(ctx);
    
    if (!cfs_rq)
        return 0;

    // Get the cgroup id from the task_group
    struct task_group *tg = BPF_CORE_READ(cfs_rq, tg);
    if (!tg)
        return 0;

    struct cgroup_subsys_state *css = &tg->css;
    if (!css)
        return 0;

    u64 cgroup_id = BPF_CORE_READ(css, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS)
        return 0;

    // Increment throttled count
    array_incr(&throttled_count, cgroup_id);
    
    return 0;
}

SEC("kprobe/unthrottle_cfs_rq")
int unthrottle_cfs_rq(struct pt_regs *ctx)
{
    struct cfs_rq *cfs_rq = (struct cfs_rq *)PT_REGS_PARM1(ctx);
    
    if (!cfs_rq)
        return 0;

    // Get the cgroup id from the task_group
    struct task_group *tg = BPF_CORE_READ(cfs_rq, tg);
    if (!tg)
        return 0;

    struct cgroup_subsys_state *css = &tg->css;
    if (!css)
        return 0;

    u64 cgroup_id = BPF_CORE_READ(css, id);
    if (!cgroup_id || cgroup_id >= MAX_CGROUPS)
        return 0;
        
    // Calculate throttling duration
    u64 now = bpf_ktime_get_ns();
    u64 throttled_at = BPF_CORE_READ(cfs_rq, throttled_clock);
    u64 duration = now - throttled_at;
    
    // Add to throttled time counter
    array_add(&throttled_time, cgroup_id, duration);
    
    return 0;
}

char LICENSE[] SEC("license") = "GPL";