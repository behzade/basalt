// String manipulation example
import std::io;
import std::strings;

let message = "Hello, World!";
let separator = " ";

io.print("Original message:", message);

// Test contains function
let hasHello = strings.contains(message, "Hello");
let hasGoodbye = strings.contains(message, "Goodbye");

io.print("Contains 'Hello':", hasHello);
io.print("Contains 'Goodbye':", hasGoodbye);

// Test split function
let words = strings.split(message, separator);
io.print("Split by space:", words);

// Test join function
let joined = strings.join(words, "-");
io.print("Joined with '-':", joined);

// More examples
let csv = "apple,banana,cherry";
let fruits = strings.split(csv, ",");
io.print("CSV fruits:", fruits);

let sentence = strings.join(fruits, " and ");
io.print("Sentence:", sentence); 