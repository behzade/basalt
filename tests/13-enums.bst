// Test 13: Enums
// Testing enum definitions and pattern matching

enum Option {
    Some(i64),
    None,
}

let someValue = Option::Some(42);
let noneValue = Option::None;

let result = match someValue {
    Some(x) => x,
    None => 0,
}; 