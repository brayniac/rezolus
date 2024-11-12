#include <bpf/bpf_helpers.h>

static __always_inline void array_add(int array, u32 idx, u64 value) {
    u64 *cnt;

    cnt = bpf_map_lookup_elem(array, &idx);

    if (cnt) {
        __atomic_fetch_add(cnt, 1, __ATOMIC_RELAXED);
    }
}

static __always_inline void array_incr(int array, u32 idx) {
    array_add(array, idx, 1);
}
