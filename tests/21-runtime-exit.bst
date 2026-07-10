import {
    std::runtime
}

// The std runtime declaration must dispatch to the host exit implementation.
fn main() -> i32 {
    runtime::exit(37)
    0
}
