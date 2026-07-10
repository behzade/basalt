fn overwrite(mut left: str, mut right: str) {
    left = "left"
    right = "right"
}

fn main() -> i32 {
    let mut value = "value"
    overwrite(value, value)
    value.len()
}
