import {
    std::runtime::raw
    std::runtime::allocator
}

pub fn raw_memory_regression() -> i32 {
    unsafe {
        let source = raw::libc_alloc(64u64)
        let destination = raw::libc_alloc(64u64)
        raw::libc_memset(source, 42u8, 64u64)
        raw::libc_memset(destination, 0u8, 64u64)
        raw::libc_memcpy(destination, source, 64u64)
        let comparison = raw::libc_memcmp(source, destination, 64u64)
        raw::libc_free(destination, 64u64)
        raw::libc_free(source, 64u64)
        if comparison == 0 { 43 } else { 1 }
    }
}

pub fn arena_regression() -> i32 {
    let mut storage = allocator::arena(32u64)
    unsafe {
        let null = raw::address_null()
        let first = allocator::arena_alloc(storage, 3u64, 1u64)
        let aligned = allocator::arena_alloc(storage, 4u64, 8u64)
        let expected_aligned = raw::address_add(first, 8u64)
        let exhausted = allocator::arena_alloc(storage, 25u64, 1u64)
        let invalid_alignment = allocator::arena_alloc(storage, 1u64, 0u64)
        allocator::arena_reset(storage)
        let reused = allocator::arena_alloc(storage, 32u64, 1u64)
        allocator::arena_release(storage)
        let after_release = allocator::arena_alloc(storage, 1u64, 1u64)

        if aligned == expected_aligned {
            if exhausted == null {
                if invalid_alignment == null {
                    if reused == first {
                        if after_release == null { 43 } else { 1 }
                    } else { 1 }
                } else { 1 }
            } else { 1 }
        } else { 1 }
    }
}

pub fn use_after_free_regression() {
    unsafe {
        let address = raw::libc_alloc(8u64)
        raw::libc_free(address, 8u64)
        raw::libc_memset(address, 1u8, 1u64)
    }
}

pub fn address_bounds_regression() {
    unsafe {
        let address = raw::libc_alloc(8u64)
        let outside = raw::address_add(address, 9u64)
        raw::libc_free(address, 8u64)
    }
}

pub fn double_free_regression() {
    unsafe {
        let address = raw::libc_alloc(8u64)
        raw::libc_free(address, 8u64)
        raw::libc_free(address, 8u64)
    }
}

pub fn copy_bounds_regression() {
    unsafe {
        let source = raw::libc_alloc(8u64)
        let destination = raw::libc_alloc(4u64)
        raw::libc_memcpy(destination, source, 8u64)
    }
}

pub fn interior_free_regression() {
    unsafe {
        let address = raw::libc_alloc(8u64)
        let interior = raw::address_add(address, 1u64)
        raw::libc_free(interior, 8u64)
    }
}
