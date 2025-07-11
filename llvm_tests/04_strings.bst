// String operations test
let greeting = "Hello";
let target = "World";

// Test string printing
print("String Literals:");
print(greeting);
print(target);

// Test string concatenation
let message = greeting + ", " + target + "!";
print(message);

// Test string comparisons
print(greeting == "Hello");    // true
print(greeting == "hello");    // false
print(target != "Earth");      // true

// Test complex string operations
let combined = (greeting + " " + target);
print(combined == "Hello World");  // true
print(combined != "Hello World");  // false 