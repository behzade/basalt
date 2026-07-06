struct Empty {}

type A = Same: Empty | Other: Empty
type B = Same: Empty | Other: Empty

fn main() -> i32 {
    let value = Same(Empty{})
    0
}
