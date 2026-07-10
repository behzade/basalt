fn main() -> i32 {
    memory Scratch: chunk(24, 1)
    let first in Scratch = "11111111111111111111"
    reset Scratch
    let second in Scratch = "22222222222222222222"
    second.len()
}
