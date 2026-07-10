// Raw process-memory primitives. Callers must hold lexical unsafe authority.
pub extern fn raw_alloc(bytes: u64) -> u64

pub extern fn raw_free(ptr: u64, bytes: u64)

pub extern fn raw_memset(ptr: u64, value: u8, bytes: u64)

pub extern fn raw_memcpy(destination: u64, source: u64, bytes: u64)

pub extern fn raw_memcmp(left: u64, right: u64, bytes: u64) -> i32
