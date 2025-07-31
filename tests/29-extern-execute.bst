extern "wasi_snapshot_preview1" {
    fn fd_write(fd: i32, iovs_ptr: i32, iovs_len: i32, nwritten_ptr: i32) -> i32
    fn memory_grow(memory_index: i32, delta: i32) -> i32
}


fn println(s: string) -> unit {
    fd_write(1, s, 1, 0)
}
