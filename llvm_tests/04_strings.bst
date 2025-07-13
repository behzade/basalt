// String operations test
import Std::Fmt;
let greeting = "Hello";
let target = "World";

Fmt.print_string("String Literals:");
Fmt.print_string(greeting);
Fmt.print_string(target);

// Test string concatenation
let message = greeting + ", " + target + "!";
Fmt.print_string(message);

// Test string comparisons
Fmt.print_bool(greeting == "Hello");    // true
Fmt.print_bool(greeting == "hello");    // false
Fmt.print_bool(target != "Earth");      // true

// Test complex string operations
let combined = (greeting + " " + target);
Fmt.print_bool(combined == "Hello World");  // true
Fmt.print_bool(combined != "Hello World");  // false 
