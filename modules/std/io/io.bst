// Use the new extern block to declare an import from the WASI host.
extern "wasi_snapshot_preview1" {
    fn fd_write(fd: i32, iovs_ptr: i32, iovs_len: i32, nwritten_ptr: i32) -> i32;
    fn memory_grow(memory_index: i32, delta: i32) -> i32;
}


pub fn println(s: string) -> unit {
    return unit;
} 
