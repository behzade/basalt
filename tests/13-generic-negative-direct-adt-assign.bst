struct Empty {}

type Option<T> = Some: T | None: Empty

fn main() -> i32 {
    let value: Option<i32> = Some(1)
    let bad: Option<str> = value
    0
}
