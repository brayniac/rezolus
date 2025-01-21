// SPDX-License-Identifier: GPL-2.0
// Copyright (c) 2024 The Rezolus Authors

#include <vmlinux.h>
#include "../../../common/bpf/cgroup_info.h"
#include "../../../common/bpf/helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#define COUNTER_GROUP_WIDTH 16
#define MAX_CPUS 1024
#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 32768

#define IDLE_STAT_INDEX 5
#define IOWAIT_STAT_INDEX 6

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

// cpu usage stat index (https://elixir.bootlin.com/linux/v6.9-rc4/source/include/linux/kernel_stat.h#L20)
// 0 - busy total
// 1 - user
// 2 - nice
// 3 - system
// 4 - softirq
// 5 - irq
//   - idle - *NOTE* this will not increment. User-space must calculate it. This index is skipped
//   - iowait - *NOTE* this will not increment. This index is skipped
// 6 - steal
// 7 - guest
// 8 - guest_nice
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CPUS * COUNTER_GROUP_WIDTH);
} counters SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_user SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(map_flags, BPF_F_MMAPABLE);
	__type(key, u32);
	__type(value, u64);
	__uint(max_entries, MAX_CGROUPS);
} cgroup_system SEC(".maps");

int account_delta(u64 delta, u32 usage_idx)
{
	u32 idx;

	if (usage_idx < COUNTER_GROUP_WIDTH) {
		// increment busy total
		idx = COUNTER_GROUP_WIDTH * bpf_get_smp_processor_id();
		array_add(&counters, idx, delta);

		// increment counter for this usage category
		idx = idx + usage_idx;
		array_add(&counters, idx, delta);
	}

	return 0;
}

SEC("kprobe/cpuacct_account_field")
int BPF_KPROBE(cpuacct_account_field_kprobe, struct task_struct *task, u32 index, u64 delta)
{
  // ignore both the idle and the iowait counting since both count the idle time
  // https://elixir.bootlin.com/linux/v6.9-rc4/source/kernel/sched/cputime.c#L227
	if (index == IDLE_STAT_INDEX || index == IOWAIT_STAT_INDEX) {
		return 0;
	}

	if (index < 2 && bpf_core_field_exists(task->sched_task_group)) {
		int cgroup_id = task->sched_task_group->css.id;
		// u64	serial_nr = task->sched_task_group->css.serial_nr;

		if (cgroup_id && cgroup_id < MAX_CGROUPS) {
			u64 *elem;

			// we check to see if this is a new cgroup by checking the serial number

			elem = bpf_map_lookup_elem(&cgroup_serial_numbers, &cgroup_id);

		// 	if (elem && *elem != serial_nr) {
		// 		// zero the counters, they will not be exported until they are non-zero
		// 		u64 zero = 0;
		// 		bpf_map_update_elem(&cgroup_user, &cgroup_id, &zero, BPF_ANY);
		// 		bpf_map_update_elem(&cgroup_system, &cgroup_id, &zero, BPF_ANY);

		// 		// initialize the cgroup info
		// 		struct cgroup_info cginfo = {
		// 			.id = cgroup_id,
		// 		};

		// 		// read the cgroup name
		// 		bpf_probe_read_kernel_str(&cginfo.name, CGROUP_NAME_LEN, task->sched_task_group->css.cgroup->kn->name);

		// 		// read the cgroup parent name
		// 		bpf_probe_read_kernel_str(&cginfo.pname, CGROUP_NAME_LEN, task->sched_task_group->css.cgroup->kn->parent->name);

		// 		// read the cgroup grandparent name
		// 		bpf_probe_read_kernel_str(&cginfo.gpname, CGROUP_NAME_LEN, task->sched_task_group->css.cgroup->kn->parent->parent->name);

		// 		// push the cgroup info into the ringbuf
		// 		bpf_ringbuf_output(&cgroup_info, &cginfo, sizeof(cginfo), 0);

		// 		// update the serial number in the local map
		// 		bpf_map_update_elem(&cgroup_serial_numbers, &cgroup_id, &serial_nr, BPF_ANY);
		// 	}

		// 	if (index == 0) {
		// 		array_add(&cgroup_user, cgroup_id, delta);
		// 	} else if (index == 1) {
		// 		array_add(&cgroup_system, cgroup_id, delta);
		// 	}
		}
	}

	// we pack the counters by skipping over the index values for idle and iowait
	// this prevents having those counters mapped to non-incrementing values in
	// this BPF program
	if (index < IDLE_STAT_INDEX) {
		return account_delta(delta, index + 1);
	} else {
		return account_delta(delta, index - 1);
	}
}

char LICENSE[] SEC("license") = "GPL";
