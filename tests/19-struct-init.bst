// Test: Struct Initialization
// This tests the lowering of struct initialization to MIR

struct Point {
    x: i64,
    y: i64,
}

struct Person {
    name: string,
    age: i64,
    active: bool,
}

fn main() -> Point {
    return Point { x: 10, y: 20 };
}

fn create_person() -> Person {
    return Person {
        name: "Alice",
        age: 30,
        active: true
    };
}

fn nested_struct() -> Point {
    let p = Point { x: 5, y: 15 };
    return p;
} 