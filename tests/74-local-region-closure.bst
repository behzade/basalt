fn main() -> i32 {
    memory scratch: chunk(64, 4)
    let allocate = fn() -> i32 {
        let value in scratch = "local"
        value.len()
    }
    allocate()
}
