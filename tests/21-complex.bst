// Test 21: Complex
// Testing complex expressions and operations

struct Point {
    x: i32,
    y: i32
}

fn distance(p1: Point, p2: Point) -> i32 {
    let dx = p1.x - p2.x
    let dy = p1.y - p2.y
    return dx * dx + dy * dy
}

fn main() -> i32 {
    let p1 = Point { x: 0, y: 0 }
    let p2 = Point { x: 3, y: 4 }
    let dist = distance(p1, p2)
    return 0
} 
