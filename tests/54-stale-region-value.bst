memory Scratch: chunk(64, 4)

fn main() -> i32 {
    let stale in Scratch = "old"
    reset Scratch
    stale.len()
}
