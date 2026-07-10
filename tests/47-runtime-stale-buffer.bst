import {
    std::buffer
}

fn main() -> i32 {
    let mut owner = buffer::buffer(1u64)
    buffer::buffer_push(owner, 42u8)
    let stale = owner
    buffer::buffer_release(owner)
    buffer::buffer_get(stale, 0u64)
    0
}
