// Test 16: Struct initialization
// Testing struct initialization with different field types

struct Person {
    name: string,
    age: i32,
    active: bool
}

fn main() -> i32 {
    let person = Person {
        name: "Alice",
        age: 30,
        active: true
    };
    return 0;
} 