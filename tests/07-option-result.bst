struct Empty {}

type Option<T> = Some: T | None: Empty

type Result<T, E> = Ok: T | Err: E

fn unwrap_or(value: Option<i32>, fallback: i32) -> i32 {
    match value
        | some: Some -> some
        | _ -> fallback
}

fn add_if_present(lhs: Option<i32>, rhs: Option<i32>) -> i32 {
    let left = unwrap_or(lhs, 0)
    let right = unwrap_or(rhs, 0)
    left + right
}

fn score(result: Result<i32, str>) -> i32 {
    match result
        | ok: Ok -> ok
        | err: Err -> err.len()
}

fn main() -> i32 {
    let present: Option<i32> = Some(7)
    let missing: Option<i32> = Option::None{}
    let total = add_if_present(present, missing)

    let ok: Result<i32, str> = Ok(5)
    total + score(ok)
}
