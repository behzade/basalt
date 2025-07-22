// Test 16e: If False with Result
// Testing if false blocks with Result returns

enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn testIfFalseResult() -> Result<i64, string> {
    if false {
        return Result::Err("error");
    }
    return Result::Ok(42);
}

let result = testIfFalseResult(); 