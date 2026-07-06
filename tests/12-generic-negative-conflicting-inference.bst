fn same<T>(left: T, right: T) -> T {
    left
}

fn main() -> i32 {
    same(1, "bad")
}
