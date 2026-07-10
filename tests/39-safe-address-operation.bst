import {
    std::runtime::allocator
}

fn main() -> i32 {
    let mut storage = allocator::arena(16u64)
    allocator::arena_alloc(storage, 1u64, 1u64) == allocator::arena_alloc(storage, 1u64, 1u64)
    allocator::arena_release(storage)
    0
}
