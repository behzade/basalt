import {
    std::runtime::testing::memory_checks
}

// Allocation policy, alignment, exhaustion, and reset are implemented in Basalt.
fn main() -> i32 {
    memory_checks::arena_regression()
}
