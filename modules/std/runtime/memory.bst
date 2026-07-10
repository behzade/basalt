pub extern fn alloc(bytes: u64) -> u64

pub extern fn free(ptr: u64, bytes: u64)

pub extern fn memset(ptr: u64, value: u8, bytes: u64)

pub extern fn memcpy(destination: u64, source: u64, bytes: u64)

pub extern fn memcmp(left: u64, right: u64, bytes: u64) -> i32
