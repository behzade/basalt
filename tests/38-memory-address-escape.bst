import {
    std::runtime
}

fn main() -> i32 {
    let escaped = unsafe {
        runtime::libc_alloc(8u64)
    }
    0
}
