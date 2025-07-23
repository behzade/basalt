// Test: Map Expressions
// This tests the lowering of map expressions to MIR

fn main() -> Map<string, i64> {
    return {"one": 1, "two": 2, "three": 3};
}

fn empty_map() -> Map<i64, string> {
    return {};
}

fn nested_maps() -> Map<string, Map<string, i64>> {
    return {
        "first": {"a": 1, "b": 2},
        "second": {"c": 3, "d": 4}
    };
} 