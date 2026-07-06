type Result<T, E> = Ok: T | Err: E

fn map_err<T, E, F>(value: Result<T, E>, f: (E) -> F) -> Result<T, F> {
    match value
        | ok: Ok -> Ok(ok)
        | err: Err -> Err(f(err))
}

fn and_then<T, U, E>(value: Result<T, E>, f: (T) -> Result<U, E>) -> Result<U, E> {
    match value
        | ok: Ok -> f(ok)
        | err: Err -> Err(err)
}

fn parse_count(input: str) -> Result<i32, str> {
    if input.len() == 5 {
        Ok(5)
    } else {
        Err("bad")
    }
}

fn bump(value: i32) -> Result<i32, i32> {
    Ok(value + 1)
}

fn main() -> i32 {
    let parsed: Result<i32, i32> = map_err(parse_count("score"), fn(message: str) -> i32 {
        message.len()
    })
    let bumped: Result<i32, i32> = and_then(parsed, bump)

    match bumped
        | ok: Ok -> ok
        | err: Err -> err
}
