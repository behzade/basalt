// Functions example
import std::io;

// Define a simple function
let greet = fn(name: string) -> none {
    io.print("Hello,", name);
};

// Define a function with return value
let add = fn(a: int64, b: int64) -> int64 {
    return a + b;
};

// Define a recursive function
let factorial = fn(n: int64) -> int64 {
    if n < 2 {
        return 1;
    } else {
        return n * factorial(n - 1);
    }
};

// Call the functions
greet("Alice");
greet("Bob");

let sum = add(5, 3);
io.print("5 + 3 =", sum);

let fact5 = factorial(5);
io.print("5! =", fact5);

// Simple doubling function
let double = fn(x: int64) -> int64 {
    return x * 2;
};

let doubled = double(21);
io.print("21 * 2 =", doubled); 