import {
    std::runtime
}

// Internal owning initialized byte storage. Capacity may exceed length, but reads stop at length.
pub struct Buffer {
    address: runtime::MemoryAddress,
    length: u64,
    capacity: u64,
}

pub fn buffer(capacity: u64) -> Buffer {
    if capacity == 0u64 {
        Buffer {
            address: unsafe { runtime::address_null() },
            length: 0u64,
            capacity: capacity,
        }
    } else {
        unsafe {
            Buffer {
                address: runtime::libc_alloc(capacity),
                length: 0u64,
                capacity: capacity,
            }
        }
    }
}

pub fn buffer_len(buffer: Buffer) -> u64 { buffer.length }
pub fn buffer_capacity(buffer: Buffer) -> u64 { buffer.capacity }

pub fn buffer_push(mut buffer: Buffer, value: u8) -> bool {
    if buffer.length == buffer.capacity {
        false
    } else {
        unsafe {
            let destination = runtime::address_add(buffer.address, buffer.length)
            runtime::libc_memset(destination, value, 1u64)
        }
        buffer.length = buffer.length + 1u64
        true
    }
}

pub fn buffer_get(buffer: Buffer, index: u64) -> u8 {
    if index < buffer.length {
        unsafe { runtime::address_load_u8(runtime::address_add(buffer.address, index)) }
    } else {
        0u8
    }
}

pub fn buffer_set(buffer: Buffer, index: u64, value: u8) -> bool {
    if index < buffer.length {
        unsafe { runtime::libc_memset(runtime::address_add(buffer.address, index), value, 1u64) }
        true
    } else {
        false
    }
}

pub fn buffer_extend(mut destination: Buffer, source: Buffer) -> bool {
    if source.length > destination.capacity - destination.length {
        false
    } else {
        if source.length > 0u64 {
            unsafe {
                let write_at = runtime::address_add(destination.address, destination.length)
                runtime::libc_memcpy(write_at, source.address, source.length)
            }
        }
        destination.length = destination.length + source.length
        true
    }
}

pub fn buffer_equal(left: Buffer, right: Buffer) -> bool {
    if left.length != right.length {
        false
    } else {
        if left.length == 0u64 { true }
        else { unsafe { runtime::libc_memcmp(left.address, right.address, left.length) == 0 } }
    }
}

pub fn buffer_clear(mut buffer: Buffer) { buffer.length = 0u64 }

pub fn buffer_release(mut buffer: Buffer) {
    unsafe {
        runtime::libc_free(buffer.address, buffer.capacity)
        buffer.address = runtime::address_null()
    }
    buffer.length = 0u64
    buffer.capacity = 0u64
}
