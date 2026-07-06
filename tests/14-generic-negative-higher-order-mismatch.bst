fn apply<T, U>(value: T, f: (T) -> U) -> U {
    f(value)
}

fn main() -> i32 {
    apply(1, fn(text: str) -> i32 {
        text.len()
    })
}
