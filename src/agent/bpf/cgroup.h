#ifndef CGROUP_H
#define CGROUP_H

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_core_read.h>

#define CGROUP_NAME_LEN 64
#define MAX_CGROUPS 4096
#define RINGBUF_CAPACITY 262144

struct cgroup_info {
    int id;
    int level;
    u8 name[CGROUP_NAME_LEN];
    u8 pname[CGROUP_NAME_LEN];
    u8 gpname[CGROUP_NAME_LEN];
};

// Check if a cgroup is new based on serial number
static __always_inline bool is_new_cgroup(void* serial_map, u32 cgroup_id, u64 serial_nr) {
    u64* elem = bpf_map_lookup_elem(serial_map, &cgroup_id);
    return elem && *elem != serial_nr;
}

// Update serial number in map
static __always_inline void update_cgroup_serial(void* serial_map, u32 cgroup_id, u64 serial_nr) {
    bpf_map_update_elem(serial_map, &cgroup_id, &serial_nr, BPF_ANY);
}

// Zero a counter for a cgroup
static __always_inline void zero_cgroup_counter(void* counter_map, u32 cgroup_id) {
    u64 zero = 0;
    bpf_map_update_elem(counter_map, &cgroup_id, &zero, BPF_ANY);
}

// Read cgroup info from task_struct
static __always_inline int read_cgroup_info(struct task_struct* task, struct cgroup_info* info) {
    void* task_group = BPF_CORE_READ(task, sched_task_group);
    if (!task_group)
        return -1;

    struct cgroup_subsys_state* css = &((struct task_group*)task_group)->css;
    if (!css)
        return -1;

    info->id = BPF_CORE_READ(css, id);
    if (!info->id || info->id >= MAX_CGROUPS)
        return -1;

    info->level = BPF_CORE_READ(css, cgroup, level);

    // Read names
    bpf_probe_read_kernel_str(&info->name, CGROUP_NAME_LEN, BPF_CORE_READ(css, cgroup, kn, name));
    bpf_probe_read_kernel_str(&info->pname, CGROUP_NAME_LEN,
                              BPF_CORE_READ(css, cgroup, kn, parent, name));
    bpf_probe_read_kernel_str(&info->gpname, CGROUP_NAME_LEN,
                              BPF_CORE_READ(css, cgroup, kn, parent, parent, name));

    return 0;
}

// Helper to get cgroup ID from current task
static __always_inline u32 get_current_cgroup_id(void) {
    struct task_struct* current = (struct task_struct*)bpf_get_current_task();
    void* task_group = BPF_CORE_READ(current, sched_task_group);
    if (!task_group)
        return 0;

    return BPF_CORE_READ(current, sched_task_group, css.id);
}

#endif // CGROUP_H