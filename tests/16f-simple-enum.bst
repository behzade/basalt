// Test 16f: Simple Enum
// Testing basic enum construction without generics

enum Option {
    Some(i64),
    None,
}

fn testOption() -> Option {
    return Option::Some(42);
}

let result = testOption(); 