import {
    std::runtime
}

// Unsafe authority is lexical and permits direct raw runtime calls.
fn main() -> i32 {
    unsafe {
        let source = runtime::alloc(64u64)
        let destination = runtime::alloc(64u64)

        runtime::memset(source, 42u8, 64u64)
        runtime::memset(destination, 0u8, 64u64)
        runtime::memcpy(destination, source, 64u64)
        let comparison = runtime::memcmp(source, destination, 64u64)

        runtime::free(destination, 64u64)
        runtime::free(source, 64u64)

        if comparison == 0 { 43 } else { 1 }
    }
}
