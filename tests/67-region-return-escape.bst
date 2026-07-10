fn leak(mut region: Region) -> Region {
    region
}

fn main() -> i32 {
    memory scratch: chunk(64, 4)
    let escaped = leak(scratch)
    0
}
