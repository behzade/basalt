import {
    std::runtime
}

// Allocation policy, alignment, exhaustion, and reset are implemented in Basalt.
fn main() -> i32 {
    let mut storage = runtime::arena(32u64)
    unsafe {
        let null = runtime::address_null()
        let first = runtime::arena_alloc(storage, 3u64, 1u64)
        let aligned = runtime::arena_alloc(storage, 4u64, 8u64)
        let expected_aligned = runtime::address_add(first, 8u64)
        let exhausted = runtime::arena_alloc(storage, 25u64, 1u64)
        let invalid_alignment = runtime::arena_alloc(storage, 1u64, 0u64)

        runtime::arena_reset(storage)
        let reused = runtime::arena_alloc(storage, 32u64, 1u64)
        runtime::arena_release(storage)
        let after_release = runtime::arena_alloc(storage, 1u64, 1u64)

        if aligned == expected_aligned {
            if exhausted == null {
                if invalid_alignment == null {
                    if reused == first {
                        if after_release == null { 43 } else { 1 }
                    } else {
                        1
                    }
                } else {
                    1
                }
            } else {
                1
            }
        } else {
            1
        }
    }
}
