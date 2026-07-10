// A namespace prefix must participate in resolution instead of being discarded.
fn main() -> i32 {
    nonexistent::len("abc")
}
