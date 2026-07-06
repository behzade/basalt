struct Empty {}

type Option<T> = Some: T | None: Empty

fn main() -> i32 {
    let bad: Option<str> = Some(1)
    0
}
