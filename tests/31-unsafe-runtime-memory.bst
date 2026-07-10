import {
    std::runtime
}

// Unsafe authority is lexical and permits direct raw runtime calls.
fn main() -> i32 {
    unsafe {
        let source = runtime::raw_alloc(64u64)
        let destination = runtime::raw_alloc(64u64)

        runtime::raw_memset(source, 42u8, 64u64)
        runtime::raw_memset(destination, 0u8, 64u64)
        runtime::raw_memcpy(destination, source, 64u64)
        let comparison = runtime::raw_memcmp(source, destination, 64u64)

        runtime::raw_free(destination, 64u64)
        runtime::raw_free(source, 64u64)

        if comparison == 0 { 43 } else { 1 }
    }
}
