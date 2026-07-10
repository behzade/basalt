fn helper() -> i32 {
    let message in scratch = "unavailable"
    message.len()
}

fn main() -> i32 {
    memory scratch: chunk(64, 4)
    helper()
}
