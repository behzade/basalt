import {
    std::runtime
}

fn main() -> i32 {
    unsafe {
        let address = runtime::libc_alloc(8u64)
        let outside = runtime::address_add(address, 9u64)
        runtime::libc_free(address, 8u64)
        0
    }
}
