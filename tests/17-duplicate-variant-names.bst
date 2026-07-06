struct Empty {}

type A = Same: Empty | Other: Empty
type B = Same: Empty | Other: Empty

fn score_a(value: A) -> i32 {
    match value
        | same: Same -> 1
        | _ -> 0
}

fn score_b(value: B) -> i32 {
    match value
        | same: Same -> 2
        | _ -> 0
}

fn main() -> i32 {
    let a: A = A::Same{}
    let b: B = B::Same{}
    score_a(a) + score_b(b)
}
