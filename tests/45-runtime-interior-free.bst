import {
    std::runtime
}

fn main() -> i32 {
    unsafe {
        let address = runtime::libc_alloc(8u64)
        let interior = runtime::address_add(address, 1u64)
        runtime::libc_free(interior, 8u64)
        0
    }
}
