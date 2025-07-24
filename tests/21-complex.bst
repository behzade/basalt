// Test 21: Complex
// Testing complex control flow with multiple features

struct Point {
    x: i32,
    y: i32
}

fn distance(p1: Point, p2: Point) -> f64 {
    let dx = p1.x - p2.x;
    let dy = p1.y - p2.y;
    return (dx * dx + dy * dy) as f64;
}

fn main() -> i32 {
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };
    
    let dist = distance(p1, p2);
    
    let result = if dist > 5.0 {
        "far"
    } else {
        "close"
    };
    
    let mut count = 0;
    while count < 3 {
        count = count + 1;
    }
    
    return 0;
} 