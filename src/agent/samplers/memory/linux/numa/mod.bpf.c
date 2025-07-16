// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2025 The Rezolus Authors

// This BPF program collects NUMA memory statistics by hooking into
// the kernel's zone statistics functions.

#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#define MAX_NUMA_NODES 1024

// Per-node NUMA event counters
// For now we'll use a single entry (index 0) for global stats
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_NUMA_NODES);
} numa_hit SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_NUMA_NODES);
} numa_miss SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_NUMA_NODES);
} numa_foreign SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_NUMA_NODES);
} numa_interleave SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_NUMA_NODES);
} numa_local SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, MAX_NUMA_NODES);
} numa_other SEC(".maps");

// Hook __zone_statistics which updates NUMA statistics
// This is called whenever pages are allocated
SEC("kprobe/__zone_statistics")
int BPF_KPROBE(__zone_statistics, struct zone *preferred_zone, 
                struct zone *z, int nr_account) {
    if (!preferred_zone || !z)
        return 0;
        
    // Get the node IDs using BPF_CORE_READ
    int preferred_nid = BPF_CORE_READ(preferred_zone, node);
    int z_nid = BPF_CORE_READ(z, node);
    
    // Get current CPU's node - this is the local node
    int cpu = bpf_get_smp_processor_id();
    int local_nid = 0;  // Default to node 0 if we can't determine
    
    // In a real implementation, we'd need to look up cpu_to_node mapping
    // For now, we'll use a simplified approach
    
    u32 key = 0; // Using global stats for now
    u64 *counter;
    
    // Update appropriate NUMA counters based on allocation type
    if (z_nid == preferred_nid) {
        // NUMA_HIT - allocated on intended node
        counter = bpf_map_lookup_elem(&numa_hit, &key);
        if (counter) {
            __sync_fetch_and_add(counter, nr_account);
        }
    } else {
        // NUMA_MISS - allocated on different node than intended
        counter = bpf_map_lookup_elem(&numa_miss, &key);
        if (counter) {
            __sync_fetch_and_add(counter, nr_account);
        }
        
        // NUMA_FOREIGN - this node served allocation intended for another
        counter = bpf_map_lookup_elem(&numa_foreign, &key);
        if (counter) {
            __sync_fetch_and_add(counter, nr_account);
        }
    }
    
    // Check if allocation is local
    if (z_nid == local_nid) {
        // NUMA_LOCAL - allocated on local node
        counter = bpf_map_lookup_elem(&numa_local, &key);
        if (counter) {
            __sync_fetch_and_add(counter, nr_account);
        }
    } else {
        // NUMA_OTHER - allocated on different node than where process runs
        counter = bpf_map_lookup_elem(&numa_other, &key);
        if (counter) {
            __sync_fetch_and_add(counter, nr_account);
        }
    }
    
    return 0;
}

// Hook refresh_cpu_vm_stats to periodically check aggregated stats
// This runs in vmstat_update workqueue context
SEC("kprobe/refresh_cpu_vm_stats")
int BPF_KPROBE(refresh_cpu_vm_stats, bool do_pagesets) {
    // This function aggregates per-CPU VM statistics
    // We can use this as a periodic trigger to ensure our counters stay in sync
    // For now, this is just a placeholder
    return 0;
}

char LICENSE[] SEC("license") = "GPL";