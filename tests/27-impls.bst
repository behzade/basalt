// Test 27: Impls
// Testing impl blocks for structs

struct Counter {
    count: i32
}

impl Counter {
    fn new() -> Counter {
        return Counter { count: 0 };
    }
    
    fn increment(mut self) {
        self.count = self.count + 1;
    }
    
    fn get_count(self) -> i32 {
        return self.count;
    }
}

fn main() -> i32 {
    let mut counter = Counter::new();
    counter.increment();
    counter.increment();
    let count = counter.get_count();
    return 0;
} 