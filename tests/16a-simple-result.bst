// Test 16a: Simple Result
// Testing basic Result enum construction

enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn simpleResult() -> Result<i64, string> {
    return Result::Ok(42);
}

let result = simpleResult(); 