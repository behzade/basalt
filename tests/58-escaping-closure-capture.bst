fn make_reader() -> () -> i32 {
    let message = "captured"
    fn() -> i32 {
        message.len()
    }
}

fn main() -> i32 {
    let reader = make_reader()
    reader()
}
