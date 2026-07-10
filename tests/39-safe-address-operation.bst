import {
    std::runtime
}

fn main() -> i32 {
    let mut storage = runtime::arena(16u64)
    runtime::arena_alloc(storage, 1u64, 1u64) == runtime::arena_alloc(storage, 1u64, 1u64)
    runtime::arena_release(storage)
    0
}
