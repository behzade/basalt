fn main() -> i32 {
    memory Source: chunk(64, 4)
    memory Destination: chunk(64, 4)
    let original in Source = "copy"
    let copied in Destination = original
    reset Source
    copied.len()
}
