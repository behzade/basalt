// libc boundary plus the minimal address operations needed by the Basalt allocator.
pub extern fn libc_alloc(bytes: u64) -> MemoryAddress
pub extern fn libc_free(address: MemoryAddress, bytes: u64)
pub extern fn libc_memset(address: MemoryAddress, value: u8, bytes: u64)
pub extern fn libc_memcpy(destination: MemoryAddress, source: MemoryAddress, bytes: u64)
pub extern fn libc_memcmp(left: MemoryAddress, right: MemoryAddress, bytes: u64) -> i32
pub extern fn address_null() -> MemoryAddress
pub extern fn address_add(address: MemoryAddress, bytes: u64) -> MemoryAddress
pub extern fn address_load_u8(address: MemoryAddress) -> u8
