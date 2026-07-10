import {
    std::runtime::raw
}

// Raw allocation intrinsics cannot escape lexical unsafe authority as values.
fn main() -> i32 {
    let allocate = raw::libc_alloc
    let release = raw::libc_free
    let ptr = allocate(64u64)
    release(ptr, 64u64)
    0
}
