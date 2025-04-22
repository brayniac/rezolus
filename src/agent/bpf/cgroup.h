#ifndef CGROUP_H
#define CGROUP_H

#define CGROUP_NAME_LEN 64

struct cgroup_info {
	int id;
	int level;
	u8 name[CGROUP_NAME_LEN];
	u8 pname[CGROUP_NAME_LEN];
	u8 gpname[CGROUP_NAME_LEN];
};

#include "helpers.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_tracing.h>

#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

// helper to extract and format cgroup info
static inline void extract_cgroup_info(struct task_struct *task, int cgroup_id, 
                                      struct cgroup_info *cginfo) {
    cginfo->id = cgroup_id;
    cginfo->level = BPF_CORE_READ(task, sched_task_group, css, cgroup, level);
    
    // Read the cgroup name hierarchy
    bpf_probe_read_kernel_str(&cginfo->name, CGROUP_NAME_LEN,
        BPF_CORE_READ(task, sched_task_group, css, cgroup, kn, name));
    bpf_probe_read_kernel_str(&cginfo->pname, CGROUP_NAME_LEN,
        BPF_CORE_READ(task, sched_task_group, css, cgroup, kn, parent, name));
    bpf_probe_read_kernel_str(&cginfo->gpname, CGROUP_NAME_LEN,
        BPF_CORE_READ(task, sched_task_group, css, cgroup, kn, parent, parent, name));
}

// cgroup counters and send info to userspace
static inline void handle_new_cgroup(void *cgroup_serial_numbers_map, 
                                    void *cgroup_info_ringbuf,
                                    int cgroup_id, u64 serial_nr,
                                    struct task_struct *task,
                                    void **counter_maps, int num_maps) {
    // Initialize all counter maps passed
    for (int i = 0; i < num_maps; i++) {
        if (counter_maps[i]) {
            u64 zero = 0;
            bpf_map_update_elem(counter_maps[i], &cgroup_id, &zero, BPF_ANY);
        }
    }

    // Initialize the cgroup info
    struct cgroup_info cginfo = {0};
    extract_cgroup_info(task, cgroup_id, &cginfo);
    
    // Push the cgroup info into the ringbuf
    bpf_ringbuf_output(cgroup_info_ringbuf, &cginfo, sizeof(cginfo), 0);
    
    // Update the serial number in the local map
    bpf_map_update_elem(cgroup_serial_numbers_map, &cgroup_id, &serial_nr, BPF_ANY);
}

#endif // CGROUP_COMMON_H