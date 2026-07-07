import {
    std::runtime
}

fn main() -> i32 {
    let ptr = runtime::alloc(64u64)
    runtime::free(ptr, 64u64)
    0
}
