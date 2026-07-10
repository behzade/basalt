fn main() -> i32 {
    let escaped = {
        memory scratch: chunk(64, 4)
        fn() -> i32 {
            reset scratch
            0
        }
    }
    escaped()
}
