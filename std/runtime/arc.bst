// ARC header structure - stored before the actual allocated memory
let ArcHeader = struct {
    ref_count: int64,
    size: int64,
};

// Result type for memory allocation
let AllocResult = enum {
    Ok(rawptr),
    Error,
};

// Allocate memory with ARC tracking - returns Result with pointer to user data
let arc_alloc = fn(size: int64) -> AllocResult {
    unsafe {
        // Allocate space for header + actual data
        let header_size = 16;
        let total_size = header_size + size;
    
        let header_raw_ptr = malloc(total_size); // This is now a rawptr
        if header_raw_ptr == 0 {
            return AllocResult::Error; // Allocation failed
        }
    
        // Initialize the header using typed pointer
        let header: *ArcHeader = header_raw_ptr; // Unsafe cast from rawptr to *ArcHeader
        header.ref_count = 1;
        header.size = size;
    
        // Return pointer to the data section (after header)
        return AllocResult::Ok(header_raw_ptr + header_size);
    }
};

// Increase reference count
let arc_retain = fn(ptr: rawptr) -> none {
    unsafe {
        if ptr == 0 {
            return;
        }
        
        // Calculate header position (16 bytes before user data)
        let header_size = 16;
        let header_raw_ptr = ptr - header_size;
        let header: *ArcHeader = header_raw_ptr;
        
        header.ref_count = header.ref_count + 1;
    }
};

// Decrease reference count and free if needed
let arc_release = fn(ptr: rawptr) -> none {
    unsafe {
        if ptr == 0 {
            return;
        }
    
        // Calculate header position (16 bytes before user data)
        let header_size = 16;
        let header_raw_ptr = ptr - header_size;
        let header: *ArcHeader = header_raw_ptr;
    
        header.ref_count = header.ref_count - 1;
    
        if header.ref_count == 0 {
            let _ = free(header_raw_ptr); // Free the entire allocation starting from header
        }
    }
};

// Get reference count (for debugging)
let arc_get_ref_count = fn(ptr: rawptr) -> int64 {
    unsafe {
        if ptr == 0 {
            return 0;
        }
        
        // Calculate header position (16 bytes before user data)
        let header_size = 16;
        let header_raw_ptr = ptr - header_size;
        let header: *ArcHeader = header_raw_ptr;
        
        return header.ref_count;
    }
};

// Internal wrapper for compiler use - handles Result type internally
// This is a temporary bridge until the compiler can handle Result types directly
let arc_alloc_internal = fn(size: int64) -> rawptr {
    unsafe {
        // Allocate space for header + actual data
        let header_size = 16;
        let total_size = header_size + size;
    
        let header_raw_ptr = malloc(total_size); // This is now a rawptr
        if header_raw_ptr == 0 {
            return 0; // Allocation failed - unsafe context allows int->rawptr
        }
    
        // Initialize the header using typed pointer
        let header: *ArcHeader = header_raw_ptr; // Unsafe cast from rawptr to *ArcHeader
        header.ref_count = 1;
        header.size = size;
    
        // Return pointer to the data section (after header)
        return header_raw_ptr + header_size;
    }
}; 
