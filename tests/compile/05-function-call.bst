// tests/compile/05-function-call.bst
// expected: 42

fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn main() -> i64 {
    return add(20, 22);
} 