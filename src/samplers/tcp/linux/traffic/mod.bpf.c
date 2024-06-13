// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2022 Francis Laniel <flaniel@linux.microsoft.com>
// Copyright (c) 2023 The Rezolus Authors

// NOTICE: this file is based off `tcptop.bpf.c` from the BCC project
// <https://github.com/iovisor/bcc/> and has been modified for use within
// Rezolus.

// This BPF program probes TCP send and receive paths to get the number of
// segments and bytes transmitted as well as the size distributions.

#include <vmlinux.h>
#include "../../../common/bpf/histogram.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_endian.h>

#define HISTOGRAM_POWER 7

/* Taken from kernel include/linux/socket.h. */
#define AF_INET		2	/* Internet IP Protocol 	*/
#define AF_INET6	10	/* IP version 6			*/

#define TCP_RX_BYTES 0
#define TCP_TX_BYTES 1
#define TCP_RX_PACKETS 2
#define TCP_TX_PACKETS 3

// counters
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, 8192); // good for up to 1024 cores w/ 8 counters
} counters SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS_POW_7);
} rx_size SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, HISTOGRAM_BUCKETS_POW_7);
} tx_size SEC(".maps");

SEC("kprobe/tcp_sendmsg")
int BPF_KPROBE(tcp_sendmsg, struct sock *sk, struct msghdr *msg, size_t size)
{
	u16 family;
	u64 *cnt;
	u32 idx, cpu_idx;

	family = BPF_CORE_READ(sk, __sk_common.skc_family);

	/* drop */
	if (family != AF_INET && family != AF_INET6) {
		return 0;
	}

	cpu_idx = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();

	idx = cpu_idx + TCP_TX_BYTES;
	cnt = bpf_map_lookup_elem(&counters, &idx);

	if (cnt) {
		__atomic_fetch_add(cnt, (u64) size, __ATOMIC_RELAXED);
	}

	idx = cpu_idx + TCP_TX_PACKETS;
	cnt = bpf_map_lookup_elem(&counters, &idx);

	if (cnt) {
		__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
	}

	idx = value_to_index((u64) size, HISTOGRAM_POWER);
	cnt = bpf_map_lookup_elem(&tx_size, &idx);

	if (cnt) {
		__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
	}

	return 0;
}

/*
 * tcp_recvmsg() would be obvious to trace, but is less suitable because:
 * - we'd need to trace both entry and return, to have both sock and size
 * - misses tcp_read_sock() traffic
 * we'd much prefer tracepoints once they are available.
 */
SEC("kprobe/tcp_cleanup_rbuf")
int BPF_KPROBE(tcp_cleanup_rbuf, struct sock *sk, int copied)
{
	u16 family;
	u64 *cnt;
	u32 idx, cpu_idx;

	if (copied <= 0) {
		return 0;
	}

	family = BPF_CORE_READ(sk, __sk_common.skc_family);

	/* drop */
	if (family != AF_INET && family != AF_INET6) {
		return 0;
	}

	cpu_idx = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();

	idx = cpu_idx + TCP_RX_BYTES;
	cnt = bpf_map_lookup_elem(&counters, &idx);

	if (cnt) {
		__atomic_fetch_add(cnt, (u64) size, __ATOMIC_RELAXED);
	}

	idx = cpu_idx + TCP_RX_PACKETS;
	cnt = bpf_map_lookup_elem(&counters, &idx);

	if (cnt) {
		__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
	}

	idx = value_to_index((u64) size, HISTOGRAM_POWER);
	cnt = bpf_map_lookup_elem(&rx_size, &idx);

	if (cnt) {
		__atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
	}

	return 0;
}

char LICENSE[] SEC("license") = "GPL";