// File I/O example with Result handling
import std::io;

// Write to a file
let content = "Hello from Basalt!\nThis is a test file.";
let writeResult = io.write_file("test.txt", content);

// Print the result (will show Result::Ok or Result::Err)
io.print("Write result:", writeResult);

// Read from the file
let readResult = io.read_file("test.txt");

// Print the result
io.print("Read result:", readResult);

// Try to read a non-existent file
let missingResult = io.read_file("nonexistent.txt");
io.print("Missing file result:", missingResult); 