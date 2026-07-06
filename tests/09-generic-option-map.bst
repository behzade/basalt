struct Empty {}

type Option<T> = Some: T | None: Empty

fn map_option<T, U>(value: Option<T>, f: (T) -> U) -> Option<U> {
    match value
        | some: Some -> Some(f(some))
        | _ -> Option::None{}
}

fn main() -> i32 {
    let value: Option<i32> = Some(8)
    let mapped: Option<str> = map_option(value, fn(n: i32) -> str {
        "score"
    })

    match mapped
        | text: Some -> text.len()
        | _ -> 0
}
