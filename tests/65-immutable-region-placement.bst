fn misuse(region: Region) -> i32 {
    let message in region = "forbidden"
    message.len()
}

fn main() -> i32 {
    memory scratch: chunk(64, 4)
    misuse(scratch)
}
