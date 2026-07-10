fn replace_with_temporary(mut value: str) {
    value = "temporary"
}

fn main() -> i32 {
    let mut value = "caller"
    replace_with_temporary(value)
    value.len()
}
