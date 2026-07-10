fn make_message() -> str {
    "region"
}

fn main() -> i32 {
    memory Result: chunk(128, 4)
    let message in Result = make_message()
    message.len()
}
