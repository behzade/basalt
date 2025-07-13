// ARC header structure - stored before the actual allocated memory
let ArcHeader = struct {
    ref_count: int64,
    size: int64,
};

// Allocate memory with ARC tracking
let arc_alloc = fn(size: int64) -> *ArcHeader {
    unsafe {
        // Allocate space for header + actual data
        let header_size = 16;
        let total_size = header_size + size;
    
        let header_raw_ptr = malloc(total_size); // This is now a rawptr
        if header_raw_ptr == 0 {
            return 0; // Allocation failed
        }
    
        // Initialize the header using typed pointer
        let header: *ArcHeader = header_raw_ptr; // Unsafe cast from rawptr to *ArcHeader
        header.ref_count = 1;
        header.size = size;
    
        // Return pointer to the data section (after header)
        // NOTE: This pointer arithmetic is also unsafe. For now, we assume the compiler handles it.
        // A more advanced implementation might require an intrinsic.
        return header_raw_ptr + header_size;
    }
};

// Increase reference count
let arc_retain = fn(ptr: *ArcHeader) -> none {
    unsafe {
        if ptr == 0 {
            return;
        }
        
        // In this new model, ptr IS the header pointer.
        let header: *ArcHeader = ptr;
        
        header.ref_count = header.ref_count + 1;
    }
};

// Decrease reference count and free if needed
let arc_release = fn(ptr: *ArcHeader) -> none {
    unsafe {
        if ptr == 0 {
            return;
        }
    
        // In this new model, ptr IS the header pointer.
        let header: *ArcHeader = ptr;
    
        header.ref_count = header.ref_count - 1;
    
        if header.ref_count == 0 {
            free(header); // Pass the raw pointer to free
        }
    }
};

// Get reference count (for debugging)
let arc_get_ref_count = fn(ptr: *ArcHeader) -> int64 {
    unsafe {
        if ptr == 0 {
            return 0;
        }
        
        // In this new model, ptr IS the header pointer.
        let header: *ArcHeader = ptr;
        
        return header.ref_count;
    }
}; 
