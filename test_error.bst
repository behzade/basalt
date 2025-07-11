// Test program with type error
let add = fn(x: int64, y: int64): int64 {
    x + y;
};

let result = add("hello", 3);
result; 