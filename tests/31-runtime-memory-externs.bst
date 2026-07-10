import {
    std::runtime
}

// Raw allocation intrinsics cannot escape lexical unsafe authority as values.
fn main() -> i32 {
    let allocate = runtime::libc_alloc
    let release = runtime::libc_free
    let ptr = allocate(64u64)
    release(ptr, 64u64)
    0
}
