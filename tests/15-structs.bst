// Test 15: Structs
// Testing struct definitions and usage

struct Point {
    x: i32,
    y: i32
}

fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    let x = p.x;
    let y = p.y;
    return 0;
} 