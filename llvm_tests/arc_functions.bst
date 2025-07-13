// Tests passing/returning ARC objects and the #[nogc] attribute.
import Std::Fmt;

// This function takes ownership of a reference and should release it on exit.
let consume = fn(val: string) -> none {
    Fmt.print_string(val);
};

// This function leaks memory because it creates a string but ARC is disabled.
#[nogc]
let leaky_function = fn() -> none {
    let leaked_str = "I am a memory leak.";
};

Fmt.print_string("--- Function Test ---");
let my_val = "Hello from main scope!";
consume(my_val); // Passes a new reference to consume.

Fmt.print_string("Calling leaky function...");
leaky_function(); // This call should not crash but will leak memory.

Fmt.print_string("Original value should still be valid:");
Fmt.print_string(my_val); // Proves original 'my_val' was not affected. 