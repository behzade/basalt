import {
    std::runtime::testing::memory_checks
}

fn main() -> i32 {
    memory_checks::use_after_free_regression()
    0
}
