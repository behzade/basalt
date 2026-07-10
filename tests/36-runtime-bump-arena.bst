import {
    std::runtime
}

// Allocation policy, alignment, exhaustion, and reset are implemented in Basalt.
fn main() -> i32 {
    let mut storage = runtime::arena(32u64)
    let first = runtime::arena_alloc(storage, 3u64, 1u64)
    let aligned = runtime::arena_alloc(storage, 4u64, 8u64)
    let exhausted = runtime::arena_alloc(storage, 25u64, 1u64)
    let invalid_alignment = runtime::arena_alloc(storage, 1u64, 0u64)

    runtime::arena_reset(storage)
    let reused = runtime::arena_alloc(storage, 32u64, 1u64)
    runtime::arena_release(storage)
    let after_release = runtime::arena_alloc(storage, 1u64, 1u64)

    if aligned - first == 8u64 {
        if exhausted == 0u64 {
            if invalid_alignment == 0u64 {
                if reused == first {
                    if after_release == 0u64 { 43 } else { 1 }
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
