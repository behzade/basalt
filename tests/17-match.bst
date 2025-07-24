// Test 17: Match
// Testing match expressions

fn main() -> i32 {
    let value = 42;
    let result = match value {
        0 => "zero",
        42 => "forty-two",
        _ => "other"
    };
    return 0;
} 