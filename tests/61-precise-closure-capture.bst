fn main() -> i32 {
    memory Scratch: chunk(64, 4)
    let unrelated in Scratch = "stale"
    reset Scratch
    let reader = fn() -> i32 { 7 }
    reader()
}
