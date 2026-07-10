struct Message {
    text: str,
}

fn main() -> i32 {
    memory Scratch: chunk(128, 8)
    let message in Scratch = Message { text: "stale" }
    reset Scratch
    message.text.len()
}
