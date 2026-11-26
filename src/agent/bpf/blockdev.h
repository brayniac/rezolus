// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2026 The Rezolus Authors

// Per-device block I/O maps. A block layer hook has the request in hand; this
// header turns its disk into a slot and indexes counter banks and per-CPU
// histogram slices by that slot. Which disk holds which slot is decided in
// bpf/blockdevs.rs; the userspace reader is bpf/blockdev_reader.rs.

#ifndef BLOCKDEV_H
#define BLOCKDEV_H

#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>

#include "btf_read.h"
#include "core_fixes.h"

#ifndef MAX_CPUS
#define MAX_CPUS 1024
#endif

// Must match crate::agent::MAX_BLOCK_DEVICES: slot 0 ("other") and 32 disks.
#define MAX_BLOCK_DEVICES 33

// Buckets a per-device histogram slice holds: the first (36 - 3 + 1) << 3 of
// the grouping-power-3 layout, the buckets below 2^36. Latency phases are
// capped below 2^36 ns (MAX_PLAUSIBLE_SPAN_NS in the blockio program) and
// request sizes are 32-bit, so the buckets above stay empty; the reader
// publishes the full 496-bucket layout with them zero. 272 buckets are 34
// cache lines, so per-CPU slices each start on a line. Must match
// crate::agent::BLK_HIST_BUCKETS.
#define BLK_HIST_POWER 3
#define BLK_HIST_BUCKETS 272

// dev_t -> slot entries the lookup holds: a disk per slot, each NVMe path of a
// multipath namespace, and disks beyond the cap mapped to slot 0. Must match
// crate::agent::bpf::blockdev_reader::LOOKUP_ENTRIES.
#define BLK_LOOKUP_ENTRIES 1024

// Disks reported to userspace for a slot. Must match
// crate::agent::bpf::blockdev_reader::UNKNOWN_ENTRIES.
#define BLK_UNKNOWN_ENTRIES 64
#define BLK_UNKNOWN_SHIFT 6  // log2(BLK_UNKNOWN_ENTRIES)
#define BLK_NAME_LEN 32

// dev_t -> slot. Written by userspace; BPF only reads it. A HASH rather than an
// ARRAY (principle 5) because dev_t is a sparse 32-bit key (12 bits of major
// above 20 of minor); the principle's objection to hash maps, update
// contention on a hot path, does not apply to a map BPF never updates.
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __type(key, u32);
    __type(value, u32);
    __uint(max_entries, BLK_LOOKUP_ENTRIES);
} blk_slots SEC(".maps");

// A disk the lookup does not hold, as reported by the request path: its
// dev_t, its name, and `check`, an FNV-1a hash of the two that lets the reader
// reject an entry two CPUs wrote at once. `dev` 0 is an empty entry.
struct blk_unknown {
    u32 dev;
    u32 check;
    char name[BLK_NAME_LEN];
};

// Indexed by a hash of the dev_t. Written by BPF only when an entry does not
// already hold the reporting dev_t, so a disk is written once until the reader
// maps it; the reader clears an entry to have a disk reported again.
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(map_flags, BPF_F_MMAPABLE);
    __type(key, u32);
    __type(value, struct blk_unknown);
    __uint(max_entries, BLK_UNKNOWN_ENTRIES);
} blk_unknown SEC(".maps");

// FNV-1a over the dev_t's four bytes, then the name up to its NUL. The reader
// computes the same (blockdev_reader.rs `unknown_check`).
static __always_inline u32 blk_check(u32 dev, const char* name) {
    u32 h = 2166136261u;

#pragma unroll
    for (int i = 0; i < 4; i++) {
        h = (h ^ ((dev >> (8 * i)) & 0xff)) * 16777619u;
    }

#pragma unroll
    for (int i = 0; i < BLK_NAME_LEN; i++) {
        if (!name[i]) {
            break;
        }
        h = (h ^ (u8)name[i]) * 16777619u;
    }

    return h;
}

// Report `dev` to userspace unless its entry already holds it.
static __always_inline void blk_report(u32 dev, struct gendisk* disk) {
    // The high bits of a multiplicative hash: the low bits of the product
    // depend only on the low bits of the minor, which most whole disks share
    // (sda, vda and nvme0n1 are all minor 0).
    u32 idx = (dev * 2654435761u) >> (32 - BLK_UNKNOWN_SHIFT);
    struct blk_unknown* e = bpf_map_lookup_elem(&blk_unknown, &idx);

    if (!e || e->dev == dev) {
        return;
    }

    // The name is read and hashed on the stack and copied in after, so a
    // second CPU writing the same entry leaves a `check` that does not match
    // the name it left, which the reader rejects.
    char name[BLK_NAME_LEN] = {};
    BPF_CORE_READ_STR_INTO(&name, disk, disk_name);
    u32 check = blk_check(dev, name);

    __builtin_memcpy(e->name, name, BLK_NAME_LEN);
    e->check = check;
    e->dev = dev;
}

// Bumped by userspace after every change to `blk_slots`, so a CPU's cached
// mapping (`blk_cache`) is used only while the lookup still holds it. Must
// match blockdev_reader.rs.
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __type(key, u32);
    __type(value, u64);
    __uint(max_entries, 1);
} blk_generation SEC(".maps");

// Each CPU's last mapping: `entry` packs the dev_t it looked up (high 32
// bits) and the slot (low 32) into one word, so a program nested on the same
// CPU (a completion in hard IRQ inside the requeue program) cannot leave one
// disk's dev_t with another's slot; `generation` is the generation read before
// the lookup. A writer stores `entry` then `generation` and a reader loads
// `generation` then `entry`, each pair separated by a compiler barrier, so a
// nested program sees a new `entry` only with a `generation` that predates
// it. 64 bytes per CPU.
struct blk_cache {
    u64 generation;
    u64 entry;
    u8 _pad[48];
};

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __type(key, u32);
    __type(value, struct blk_cache);
    __uint(max_entries, MAX_CPUS);
} blk_cache SEC(".maps");

// Keeps the compiler from moving loads or stores across it; a BPF program
// does not migrate, and `blk_cache` is shared only by programs nested on one
// CPU, so that orders them.
#define blk_barrier() asm volatile("" ::: "memory")

// The request's disk. From the tp_btf program (`btf`), `rq` is a BTF pointer
// and the fields are plain loads; otherwise get_disk() reads them with
// bpf_probe_read_kernel().
static __always_inline struct gendisk* blk_disk(struct request* rq, bool btf) {
    if (!btf) {
        return get_disk(rq);
    }

    struct request___x* r = (void*)rq;

    if (bpf_core_field_exists(r->rq_disk)) {
        return r->rq_disk;
    }

    struct request_queue___x* q = r->q;

    return q ? q->disk : NULL;
}

// The slot for a request's disk: its mapping, or 0 ("other") for a disk the
// lookup does not hold yet, which is then reported, or for a request with no
// disk. The disk, not the partition: a request is queued on the disk's queue
// whatever partition the bio came from.
static __always_inline u32 blk_slot(struct request* rq, bool btf) {
    struct gendisk* disk = blk_disk(rq, btf);

    if (!disk) {
        return 0;
    }

    u32 major = BTF_READ(btf, disk, major);
    u32 minor = BTF_READ(btf, disk, first_minor);
    u32 dev = (major << 20) | minor;

    // Read once, before the lookup: a mapping found under this generation is
    // stored with it, so a change the reader makes after this read (which
    // bumps the generation) leaves the stored entry stale, never current.
    u32 zero = 0;
    u64* generation_ptr = bpf_map_lookup_elem(&blk_generation, &zero);
    u32 cpu = bpf_get_smp_processor_id();
    struct blk_cache* cached = bpf_map_lookup_elem(&blk_cache, &cpu);

    if (!generation_ptr || !cached) {
        u32* found = bpf_map_lookup_elem(&blk_slots, &dev);
        if (!found) {
            blk_report(dev, disk);
            return 0;
        }
        return *found < MAX_BLOCK_DEVICES ? *found : 0;
    }

    u64 generation = *generation_ptr;
    u64 cached_generation = cached->generation;
    blk_barrier();
    u64 entry = cached->entry;

    if ((u32)(entry >> 32) == dev && cached_generation == generation) {
        return (u32)entry;
    }

    u32* found = bpf_map_lookup_elem(&blk_slots, &dev);

    if (!found) {
        blk_report(dev, disk);
        return 0;
    }

    u32 slot = *found < MAX_BLOCK_DEVICES ? *found : 0;

    cached->entry = ((u64)dev << 32) | slot;
    blk_barrier();
    cached->generation = generation;

    return slot;
}

// Index of `counter` in this CPU's bank for `slot`; the reader computes the
// same (blockdev_reader.rs `bank_index`).
static __always_inline u32 blk_counter_idx(u32 slot, u32 counter, u32 width) {
    return (bpf_get_smp_processor_id() * MAX_BLOCK_DEVICES + slot) * width + counter;
}

// The bucket for `value`, or BLK_HIST_BUCKETS for a value at or above 2^36,
// which no histogram records.
static __always_inline u32 blk_bucket(u64 value) {
    u32 bucket = value_to_index(value, BLK_HIST_POWER);

    return bucket < BLK_HIST_BUCKETS ? bucket : BLK_HIST_BUCKETS;
}

// Index of `bucket` in this CPU's slice of `slot`'s histogram, in a per-device
// histogram map of MAX_CPUS x MAX_BLOCK_DEVICES slices; the reader computes the
// same (blockdev_reader.rs `slice_index`).
static __always_inline u32 blk_hist_idx(u32 slot, u32 bucket) {
    return (bpf_get_smp_processor_id() * MAX_BLOCK_DEVICES + slot) * BLK_HIST_BUCKETS + bucket;
}

#endif
