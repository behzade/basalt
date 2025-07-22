// Test 16c: While Loop
// Testing while loops with return statements

fn testWhile() -> i64 {
    let mut sum = 0;
    let mut i = 0;
    while i < 5 {
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}

let result = testWhile(); 