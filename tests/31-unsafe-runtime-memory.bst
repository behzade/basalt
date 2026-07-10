import {
    std::runtime
}

// Unsafe authority is lexical and permits direct raw runtime calls.
fn main() -> i32 {
    unsafe {
        let ptr = runtime::alloc(64u64)
        runtime::free(ptr, 64u64)
    }
    0
}
