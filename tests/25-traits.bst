// Test 25: Traits
// Testing trait definitions

trait Printable {
    fn print(self) -> unit;
}

struct Point {
    x: i32,
    y: i32
}

impl Printable for Point {
    fn print(self) -> unit {
        // In a real implementation, this would print the point
        return ();
    }
}

fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    p.print();
    return 0;
} 