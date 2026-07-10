fn main() -> i32 {
    memory Tiny: chunk(32, 4)
    let one in Tiny = "11111111111111111111"
    let two in Tiny = "22222222222222222222"
    two.len()
}
