// Use the new extern block to declare an import from the WASI host.
extern "wasi:cli/stdout" {
    fn fd_write(s: string) -> unit;
}

// Export a public, idiomatic function for users.
pub fn println(s: string) -> unit {
    fd_write(s + "\n");
} 