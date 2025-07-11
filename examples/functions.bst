// Functions example
import std::io;

// Define a simple function
let greet = fn(name) {
    io.print("Hello,", name);
};

// Define a function with return value
let add = fn(a, b) {
    return a + b;
};

// Define a recursive function
let factorial = fn(n) {
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

// Higher-order function
let apply_twice = fn(f, x) {
    return f(f(x));
};

let double = fn(x) {
    return x * 2;
};

let result = apply_twice(double, 3);
io.print("Applying double twice to 3:", result); 