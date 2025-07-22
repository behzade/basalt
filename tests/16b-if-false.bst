// Test 16b: If False
// Testing if false blocks with return statements

fn testIfFalse() -> i64 {
    if false {
        return 100;
    }
    return 42;
}

let result = testIfFalse(); 