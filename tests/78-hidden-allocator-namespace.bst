fn arena_alloc() -> i32 { 7 }

fn main() -> i32 {
    memory Scratch: chunk(64, 2)
    let value in Scratch = "region"
    arena_alloc()
}
