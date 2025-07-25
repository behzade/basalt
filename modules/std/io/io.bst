// Use the new extern block to declare an import from the WASI host.
extern "wasi_snapshot_preview1" {
    fn fd_write(fd: i32, iovs_ptr: i32, iovs_len: i32, nwritten_ptr: i32) -> i32;
}

// Export a public, idiomatic function for users.
pub fn println(s: string) -> unit {
    // For now, use fd_write with hardcoded "Hello from Std::Io!\n" string
    // TODO: Implement proper string to WASI iovec conversion
    // fd_write(1, iovs_ptr, 1, nwritten_ptr) where 1 is stdout
    // For now, just call fd_write with placeholder values
    fd_write(1, 0, 1, 0);
} 