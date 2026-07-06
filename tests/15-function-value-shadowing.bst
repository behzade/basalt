fn f() -> i32 {
    1
}

fn main() -> i32 {
    let f = fn() -> i32 {
        2
    }
    f()
}
