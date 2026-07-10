import {
    std::runtime
}

// Unsafe authority is lexical and permits direct raw runtime calls.
fn main() -> i32 {
    unsafe {
        let source = runtime::libc_alloc(64u64)
        let destination = runtime::libc_alloc(64u64)

        runtime::libc_memset(source, 42u8, 64u64)
        runtime::libc_memset(destination, 0u8, 64u64)
        runtime::libc_memcpy(destination, source, 64u64)
        let comparison = runtime::libc_memcmp(source, destination, 64u64)

        runtime::libc_free(destination, 64u64)
        runtime::libc_free(source, 64u64)

        if comparison == 0 { 43 } else { 1 }
    }
}
