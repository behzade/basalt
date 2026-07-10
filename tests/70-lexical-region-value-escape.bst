fn main() -> i32 {
    let copied = {
        memory scratch: chunk(64, 4)
        let local in scratch = "copied"
        local
    }
    copied.len()
}
