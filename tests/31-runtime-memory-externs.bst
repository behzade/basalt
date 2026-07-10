import {
    std::runtime
}

// Raw allocation intrinsics cannot escape lexical unsafe authority as values.
fn main() -> i32 {
    let allocate = runtime::raw_alloc
    let release = runtime::raw_free
    let ptr = allocate(64u64)
    release(ptr, 64u64)
    0
}
