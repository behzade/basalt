// Returning from a nested block must unwind the enclosing function.
fn main() -> i32 {
    let value: i32 = {
        return 7
        3
    }
    value + 2
}
