import {
    std::runtime
}

fn main() -> i32 {
    let mut owner = runtime::buffer(1u64)
    runtime::buffer_push(owner, 42u8)
    let stale = owner
    runtime::buffer_release(owner)
    runtime::buffer_get(stale, 0u64)
    0
}
