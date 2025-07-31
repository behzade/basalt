// Test 26: Generics
// Testing generic functions and types

fn identity<T>(x: T) -> T {
    return x
}

struct Box<T> {
    value: T
}

fn main() -> i32 {
    let x = identity(42)
    let y = identity("hello")
    let box_int = Box { value: 100 }
    let box_str = Box { value: "world" }
    return 0
} 
