fn clear(mut region: Region) {
    reset region
}

fn main() -> i32 {
    memory scratch: chunk(64, 4)
    let stale in scratch = "old"
    clear(scratch)
    let fresh in scratch = "fresh"
    fresh.len()
}
