// Test 18: Literal patterns
// Testing literal patterns in match expressions

fn main() -> i32 {
    let value = 42;
    let result = match value {
        0 => "zero",
        1 => "one",
        2 => "two",
        42 => "forty-two",
        100 => "hundred",
        _ => "other"
    };
    
    let bool_val = true;
    let bool_result = match bool_val {
        true => "yes",
        false => "no"
    };
    
    let str_val = "hello";
    let str_result = match str_val {
        "hello" => "greeting",
        "world" => "planet",
        _ => "other"
    };
    
    return 0;
} 