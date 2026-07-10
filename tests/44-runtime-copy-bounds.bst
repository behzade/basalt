import {
    std::runtime
}

fn main() -> i32 {
    unsafe {
        let source = runtime::libc_alloc(8u64)
        let destination = runtime::libc_alloc(4u64)
        runtime::libc_memcpy(destination, source, 8u64)
        0
    }
}
