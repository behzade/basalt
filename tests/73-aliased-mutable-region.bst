fn use_both(mut left: Region, mut right: Region) {
    reset left
    reset right
}

fn main() -> i32 {
    memory scratch: chunk(64, 4)
    let mut alias = scratch
    use_both(scratch, alias)
    0
}
