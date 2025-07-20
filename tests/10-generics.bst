// Test 10: Generics
// Testing generic struct definitions

struct Pair<T, U> {
    first: T,
    second: U,
}

let intPair = Pair<i64, string> {
    first: 42,
    second: "hello",
};

let floatPair = Pair<f64, bool> {
    first: 3.14,
    second: true,
}; 