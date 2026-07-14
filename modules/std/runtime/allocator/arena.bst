import {
    std::runtime::raw
}

// Allocator policy lives in Basalt. Only the raw process-memory boundary is hosted.

pub struct Arena {
    base: raw::MemoryAddress,
    capacity: u64,
    cursor: u64,
    object_limit: u64,
    object_count: u64,
}

pub fn arena(bytes: u64) -> Arena {
    arena_with_objects(bytes, 18446744073709551615u64)
}

pub fn arena_with_objects(bytes: u64, objects: u64) -> Arena {
    unsafe {
        Arena {
            base: raw::libc_alloc(bytes),
            capacity: bytes,
            cursor: 0u64,
            object_limit: objects,
            object_count: 0u64,
        }
    }
}

pub fn arena_alloc(mut arena: Arena, bytes: u64, alignment: u64) -> raw::MemoryAddress {
    if arena.object_count == arena.object_limit {
        unsafe { raw::address_null() }
    } else {
        if alignment == 0u64 {
            unsafe { raw::address_null() }
        } else {
            let remainder = arena.cursor - (arena.cursor / alignment) * alignment
            let padding = if remainder == 0u64 { 0u64 } else { alignment - remainder }
            if padding > arena.capacity - arena.cursor {
                unsafe { raw::address_null() }
            } else {
                let aligned = arena.cursor + padding
                if bytes > arena.capacity - aligned {
                    unsafe { raw::address_null() }
                } else {
                    arena.cursor = aligned + bytes
                    arena.object_count = arena.object_count + 1u64
                    unsafe { raw::address_add(arena.base, aligned) }
                }
            }
        }
    }
}

pub fn arena_reset(mut arena: Arena) {
    arena.cursor = 0u64
    arena.object_count = 0u64
}

pub fn arena_release(mut arena: Arena) {
    unsafe {
        raw::libc_free(arena.base, arena.capacity)
    }
    arena.base = unsafe { raw::address_null() }
    arena.capacity = 0u64
    arena.cursor = 0u64
    arena.object_limit = 0u64
    arena.object_count = 0u64
}
