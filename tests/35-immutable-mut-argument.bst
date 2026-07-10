struct Counter {
    value: i32,
}

fn increment(mut counter: Counter) -> i32 {
    counter.value = counter.value + 1
    counter.value
}

fn main() -> i32 {
    let counter = Counter { value: 41 }
    increment(counter)
}
