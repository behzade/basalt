import {
    regression::left
    regression::right
}

// Qualified calls with the same terminal name retain their module identity.
fn main() -> i32 {
    left::value() + right::value()
}
