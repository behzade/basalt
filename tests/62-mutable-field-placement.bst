struct Message {
    text: str,
}

fn overwrite(mut message: Message) {
    message.text = "temporary"
}

fn main() -> i32 {
    let mut message = Message { text: "caller" }
    overwrite(message)
    message.text.len()
}
