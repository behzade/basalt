// ARC header structure - stored before the actual allocated memory
let ArcHeader = struct {
    ref_count: int64,
    size: int64,
};

// Allocate memory with ARC tracking
let arc_alloc = fn(size: int64) -> array_ptr {
    // Allocate space for header + actual data
    let header_size = 16; // sizeof(ArcHeader) - 8 bytes for ref_count + 8 bytes for size
    let total_size = header_size + size;
    
    let header_ptr = malloc(total_size);
    if header_ptr == 0 {
        return 0; // Allocation failed
    }
    
    // Initialize the header using typed pointer
    let header: *ArcHeader = header_ptr;
    header.ref_count = 1;
    header.size = size;
    
    // Return pointer to the data section (after header)
    return header_ptr + header_size;
};

// Increase reference count
let arc_retain = fn(ptr: array_ptr) -> none {
    if ptr == 0 {
        return none;
    }
    
    let header_size = 16;
    let header_ptr = ptr - header_size;
    let header: *ArcHeader = header_ptr;
    
    header.ref_count = header.ref_count + 1;
};

// Decrease reference count and free if needed
let arc_release = fn(ptr: array_ptr) -> none {
    if ptr == 0 {
        return none;
    }
    
    let header_size = 16;
    let header_ptr = ptr - header_size;
    let header: *ArcHeader = header_ptr;
    
    header.ref_count = header.ref_count - 1;
    
    if header.ref_count == 0 {
        free(header_ptr);
    }
};

// Get reference count (for debugging)
let arc_get_ref_count = fn(ptr: array_ptr) -> int64 {
    if ptr == 0 {
        return 0;
    }
    
    let header_size = 16;
    let header_ptr = ptr - header_size;
    let header: *ArcHeader = header_ptr;
    
    return header.ref_count;
} 
