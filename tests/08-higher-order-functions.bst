fn apply_twice(f: (i32) -> i32, value: i32) -> i32 {
    f(f(value))
}

fn make_adder(base: i32) -> (i32) -> i32 {
    fn(value: i32) -> i32 {
        base + value
    }
}

fn main() -> i32 {
    let add_three = make_adder(3)
    apply_twice(add_three, 4)
}
