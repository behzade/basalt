import {
    std::runtime::raw
}

fn main() -> i32 {
    let escaped = unsafe {
        raw::libc_alloc(8u64)
    }
    0
}
