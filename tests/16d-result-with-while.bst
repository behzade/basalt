// Test 16d: Result with While
// Testing Result enum with while loops

enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn testResultWhile() -> Result<i64, string> {
    let mut sum = 0;
    let mut i = 0;
    while i < 5 {
        sum = sum + i;
        i = i + 1;
    }
    return Result::Ok(sum);
}

let result = testResultWhile(); 