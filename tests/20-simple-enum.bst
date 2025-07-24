// Test 20: Simple enum
// Testing simple enum with data

enum Option<T> {
    Some(T),
    None
}

fn main() -> i32 {
    let some_value = Option::Some(42);
    let none_value = Option::None;
    
    let result = match some_value {
        Option::Some(x) => x,
        Option::None => 0
    };
    
    return 0;
} 