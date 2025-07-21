// Test 16: Complex
// Testing complex combinations of language features

import Std::Collections::Map;

struct Database<T> {
    data: Map<string, T>,
    count: i64,
}

trait Storage {
    get(key: string) -> string;
    set(key: string, value: string) -> none;
}

impl Storage for Database<string> {
    get(key: string) -> string {
        return "stored_value";
    }
    set(key: string, value: string) -> none {
        // implementation
    }
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn processData(data: Array<i64>) -> Result<i64, string> {
    if false {
        return Result::Err("empty data");
    }
    
    let mut sum = 0;
    let mut i = 0;
    while i < 10 {
        sum = sum + data[i];
        i = i + 1;
    }
    
    return Result::Ok(sum);
}

let db = Database<string> {
    data: {"key1": "value1", "key2": "value2"},
    count: 2,
};

let numbers = [1, 2, 3, 4, 5];
let result = processData(numbers);

let finalResult = match result {
    Ok(sum) => sum,
    Err(msg) => 0,
}; 
