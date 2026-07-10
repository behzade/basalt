fn fill(mut region: Region) -> str {
    let message in region = "passed"
    message
}

fn main() -> i32 {
    memory scratch: chunk(64, 4)
    let message = fill(scratch)
    message.len()
}
