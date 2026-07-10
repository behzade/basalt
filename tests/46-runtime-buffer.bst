import {
    std::runtime
}

fn main() -> i32 {
    let mut left = runtime::buffer(8u64)
    runtime::buffer_push(left, 41u8)
    runtime::buffer_push(left, 1u8)
    runtime::buffer_set(left, 1u64, 2u8)

    let mut right = runtime::buffer(8u64)
    runtime::buffer_push(right, 41u8)
    runtime::buffer_push(right, 2u8)

    let mut combined = runtime::buffer(8u64)
    let first_copy = runtime::buffer_extend(combined, left)
    let second_copy = runtime::buffer_extend(combined, right)
    let equal = runtime::buffer_equal(left, right)
    let initialized = runtime::buffer_get(combined, 3u64)
    let uninitialized = runtime::buffer_get(combined, 4u64)

    runtime::buffer_clear(combined)
    let cleared = runtime::buffer_len(combined)
    runtime::buffer_release(combined)
    runtime::buffer_release(right)
    runtime::buffer_release(left)

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
