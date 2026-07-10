// Mutable captured state persists between calls to the same closure.
fn make_counter() -> () -> i32 {
    let mut count: i32 = 0
    fn() -> i32 {
        count = count + 1
        count
    }
}

fn main() -> i32 {
    let counter = make_counter()
    counter()
    counter()
}
