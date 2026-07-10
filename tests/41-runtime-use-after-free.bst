import {
    std::runtime
}

fn main() -> i32 {
    unsafe {
        let address = runtime::libc_alloc(8u64)
        runtime::libc_free(address, 8u64)
        runtime::libc_memset(address, 1u8, 1u64)
        0
    }
}
