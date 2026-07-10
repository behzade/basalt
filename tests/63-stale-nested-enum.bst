struct Payload {
    text: str,
}

type Message = Some: Payload | None: Payload

fn main() -> i32 {
    memory Scratch: chunk(128, 8)
    let message in Scratch: Message = Message::Some{ text: "stale" }
    reset Scratch
    match message
        | payload: Some -> payload.text.len()
        | _ -> 0
}
