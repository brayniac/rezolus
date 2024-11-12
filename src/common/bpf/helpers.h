#include <bpf/bpf_helpers.h>

static __always_inline void array_add(void *array, u32 idx, u64 value) {
    u64 *elem;

    elem = bpf_map_lookup_elem(array, &idx);

    if (elem) {
        __atomic_fetch_add(elem, value, __ATOMIC_RELAXED);
    }
}

static __always_inline void array_incr(void *array, u32 idx) {
    array_add(array, idx, 1);
}
