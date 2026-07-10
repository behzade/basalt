import {
    std::runtime
}

// A real write attempts to open this impossible child path and reports an error.
fn main() -> i32 {
    runtime::write("/dev/null/child", "data")
    0
}
