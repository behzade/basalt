import {
    std::runtime
}

// Raw allocation intrinsics remain inaccessible until unsafe blocks exist.
fn main() -> i32 {
    let allocate = runtime::alloc
    let release = runtime::free
    let ptr = allocate(64u64)
    release(ptr, 64u64)
    0
}
