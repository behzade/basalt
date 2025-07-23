// Test: Match Expressions
// This tests the lowering of match expressions to MIR

enum Option<T> {
    Some(T),
    None,
}

fn match_literal(x: i64) -> string {
    match x {
        1 => "one",
        2 => "two",
        3 => "three",
        _ => "other"
    }
}

fn match_binding(x: Option<i64>) -> i64 {
    match x {
        Option::Some(value) => value,
        Option::None => 0
    }
}

fn match_wildcard(x: i64) -> string {
    match x {
        42 => "answer",
        _ => "not the answer"
    }
} 