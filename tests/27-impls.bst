// Test 27: Impls
// Testing impl blocks for structs

struct Counter {
    count: i32
}

fn Counter::new() -> Counter {
    return Counter { count: 0 }
}

fn Counter::increment(mut self) {
    self.count = self.count + 1
}

fn Counter::get_count(self) -> i32 {
    return self.count
}

trait Incrementable {
    increment(self)
}

Counter satisfies Incrementable

fn main() -> i32 {
    let mut counter = Counter::new();
    counter.increment();
    counter.increment();
    let count = counter.get_count();
    return 0;
} 
