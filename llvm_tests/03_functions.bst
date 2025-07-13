// Test user-defined functions
let add = fn(x: int64, y: int64) -> int64 {
    x + y
};

let factorial = fn(n: int64) -> int64 {
    if n < 2 {
        1
    } else {
        n * factorial(n - 1)
    }
};

// Test function calls
let result1 = add(5, 3);
print(result1);

let result2 = factorial(5);
print(result2); 
