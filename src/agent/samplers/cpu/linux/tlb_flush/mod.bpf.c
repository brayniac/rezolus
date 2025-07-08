// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program tracks tlb_flush events

#include <vmlinux.h>
#include "../../../agent/bpf/cgroup.h"
#include "../../../agent/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>

#define COUNTER_GROUP_WIDTH 8
#define MAX_CPUS 1024
#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

#define REASON_TASK_SWITCH 0
#define REASON_REMOTE_SHOOTDOWN 1
#define REASON_LOCAL_SHOOTDOWN 2
#define REASON_LOCAL_MM_SHOOTDOWN 3
#define REASON_REMOTE_SEND_IPI 4

// counters for tlb_flush events
// 0 - task_switch
// 1 - remote shootdown
// 2 - local shootdown
// 3 - local mm shootdown
// 4 - remote send ipi
//
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CPUS* COUNTER_GROUP_WIDTH);
} events SEC(".maps");

/*
 * cgroup instrumentation
 */

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

// counters for various events

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_task_switch SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_remote_shootdown SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_local_shootdown SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_local_mm_shootdown SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_CGROUPS);
} cgroup_remote_send_ipi SEC(".maps");

SEC("raw_tp/tlb_flush")
int BPF_PROG(tlb_flush, int reason, u64 pages) {
    u32 offset, idx;
    u64* elem;

    offset = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();

    idx = reason + offset;

    array_incr(&events, idx);

    struct task_struct* current = (struct task_struct*)bpf_get_current_task();

    if (bpf_core_field_exists(current->sched_task_group)) {
        u64 serial_nr = BPF_CORE_READ(current, sched_task_group, css.serial_nr);
        struct cgroup_info cginfo = {};

        if (read_cgroup_info(current, &cginfo) == 0) {
            u32 cgroup_id = cginfo.id;

            // Check if this is a new cgroup by checking the serial number
            if (is_new_cgroup(&cgroup_serial_numbers, cgroup_id, serial_nr)) {
                // Zero the counters, they will not be exported until non-zero
                zero_cgroup_counter(&cgroup_task_switch, cgroup_id);
                zero_cgroup_counter(&cgroup_remote_shootdown, cgroup_id);
                zero_cgroup_counter(&cgroup_local_shootdown, cgroup_id);
                zero_cgroup_counter(&cgroup_local_mm_shootdown, cgroup_id);
                zero_cgroup_counter(&cgroup_remote_send_ipi, cgroup_id);

                // Push the cgroup info into the ringbuf
                bpf_ringbuf_output(&cgroup_info, &cginfo, sizeof(cginfo), 0);

                // Update the serial number in the local map
                update_cgroup_serial(&cgroup_serial_numbers, cgroup_id, serial_nr);
            }

            // Update cgroup counter
            switch (reason) {
            case REASON_TASK_SWITCH:
                array_incr(&cgroup_task_switch, cgroup_id);
                break;
            case REASON_REMOTE_SHOOTDOWN:
                array_incr(&cgroup_remote_shootdown, cgroup_id);
                break;
            case REASON_LOCAL_SHOOTDOWN:
                array_incr(&cgroup_local_shootdown, cgroup_id);
                break;
            case REASON_LOCAL_MM_SHOOTDOWN:
                array_incr(&cgroup_local_mm_shootdown, cgroup_id);
                break;
            case REASON_REMOTE_SEND_IPI:
                array_incr(&cgroup_remote_send_ipi, cgroup_id);
                break;
            }
        }
    }

    return 0;
}

char LICENSE[] SEC("license") = "GPL";
