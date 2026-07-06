struct Empty {}

type Choice = First: Empty | Second: Empty

fn main() -> i32 {
    let value: Choice = Choice::First{}
    match value
        | first: First -> 1
}
