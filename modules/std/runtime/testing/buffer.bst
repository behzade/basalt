import {
    std::runtime::buffer
}

// Runtime-owned executable coverage. The raw Buffer API remains in an internal submodule.
pub fn buffer_regression() -> i32 {
    let mut left = buffer::buffer(8u64)
    buffer::buffer_push(left, 41u8)
    buffer::buffer_push(left, 1u8)
    buffer::buffer_set(left, 1u64, 2u8)

    let mut right = buffer::buffer(8u64)
    buffer::buffer_push(right, 41u8)
    buffer::buffer_push(right, 2u8)

    let mut combined = buffer::buffer(8u64)
    let first_copy = buffer::buffer_extend(combined, left)
    let second_copy = buffer::buffer_extend(combined, right)
    let equal = buffer::buffer_equal(left, right)
    let initialized = buffer::buffer_get(combined, 3u64)
    let uninitialized = buffer::buffer_get(combined, 4u64)

    buffer::buffer_clear(combined)
    let cleared = buffer::buffer_len(combined)
    buffer::buffer_release(combined)
    buffer::buffer_release(right)
    buffer::buffer_release(left)

    if first_copy {
        if second_copy {
            if equal {
                if initialized == 2u8 {
                    if uninitialized == 0u8 {
                        if cleared == 0u64 { 43 } else { 1 }
                    } else { 1 }
                } else { 1 }
            } else { 1 }
        } else { 1 }
    } else { 1 }
}

pub fn stale_buffer_regression() {
    let mut owner = buffer::buffer(1u64)
    buffer::buffer_push(owner, 42u8)
    let stale = owner
    buffer::buffer_release(owner)
    let ignored = buffer::buffer_get(stale, 0u64)
}
