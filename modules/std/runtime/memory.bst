// Allocator policy lives in Basalt. Only the raw process-memory boundary is hosted.
pub struct Arena {
    base: u64,
    capacity: u64,
    cursor: u64,
}

pub fn arena(bytes: u64) -> Arena {
    unsafe {
        Arena {
            base: raw_alloc(bytes),
            capacity: bytes,
            cursor: 0u64,
        }
    }
}

pub fn arena_alloc(mut arena: Arena, bytes: u64, alignment: u64) -> u64 {
    if alignment == 0u64 {
        0u64
    } else {
        let remainder = arena.cursor - (arena.cursor / alignment) * alignment
        let padding = if remainder == 0u64 { 0u64 } else { alignment - remainder }
        if padding > arena.capacity - arena.cursor {
            0u64
        } else {
            let aligned = arena.cursor + padding
            if bytes > arena.capacity - aligned {
                0u64
            } else {
                arena.cursor = aligned + bytes
                arena.base + aligned
            }
        }
    }
}

pub fn arena_reset(mut arena: Arena) {
    arena.cursor = 0u64
}

pub fn arena_release(mut arena: Arena) {
    unsafe {
        raw_free(arena.base, arena.capacity)
    }
    arena.base = 0u64
    arena.capacity = 0u64
    arena.cursor = 0u64
}
