// A selected match arm mutates the surrounding lexical environment.
fn main() -> i32 {
    let mut result: i32 = 0
    match true
        | true -> {
            result = 41
            0
        }
        | _ -> 0
    result
}
