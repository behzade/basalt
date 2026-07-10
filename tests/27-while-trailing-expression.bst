// A loop body value is discarded; it must not terminate the loop.
fn main() -> i32 {
    let mut value: i32 = 43
    while value > 40 {
        value = value - 1
        value
    }
    value
}
