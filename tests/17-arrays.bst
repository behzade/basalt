// Test: Array Expressions
// This tests the lowering of array expressions to MIR

fn main() -> Array<i64> {
    return [1, 2, 3, 4, 5];
}

fn empty_array() -> Array<string> {
    return [];
}

fn nested_arrays() -> Array<Array<i64>> {
    return [[1, 2], [3, 4], [5]];
} 