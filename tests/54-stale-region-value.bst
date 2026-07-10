fn main() -> i32 {
    memory Scratch: chunk(64, 4)
    let stale in Scratch = "old"
    reset Scratch
    stale.len()
}
