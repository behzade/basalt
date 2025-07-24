// Use the new extern block to declare an import from the WASI host.
extern "wasi:cli/stdout" {
    fn write(s: string) -> none;
}

// Export a public, idiomatic function for users.
pub fn println(s: string) -> none {
    write(s + "\n");
} 