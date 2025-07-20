// Test 09: Impls
// Testing trait implementations

struct Calculator {
    base: i64,
}

trait Addable {
    add(a: i64, b: i64) -> i64;
}

impl Addable for Calculator {
    add(a: i64, b: i64) -> i64 {
        return a + b;
    }
} 
