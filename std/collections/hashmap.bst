import Std::Runtime;

// An entry in the hash map
let Entry<K, V> = struct {
    key: K,
    value: V,
    hash: int64,
    is_occupied: bool,
};

// The HashMap itself
let HashMap<K, V> = struct {
    // Pointer to a heap-allocated array of Entry<K, V>
    entries: *Entry<K, V>,
    capacity: int64,
    count: int64,
};

// Private helper function for hashing strings (FNV-1a algorithm)
let _hash_string = fn(key: string) -> int64 {
    let offset_basis = 14695981039346656037; // 64-bit FNV offset basis
    let fnv_prime = 1099511628211;         // 64-bit FNV prime

    let mut hash = offset_basis;
    let len = Std::Runtime.strlen(key);
    let mut i = 0;
    unsafe {
        let key_ptr: rawptr = key;
        for i < len {
            let byte_val = *(key_ptr + i); // Get byte at position i
            hash = hash ^ byte_val;
            hash = hash * fnv_prime;
            i = i + 1;
        }
    }
    return hash;
};

// Private helper function for hashing int64 values
let _hash_int64 = fn(key: int64) -> int64 {
    // Simple multiplicative hash for int64
    let mut hash = key;
    hash = hash * 2654435761; // Large prime number
    hash = hash ^ (hash >> 16);
    hash = hash * 2654435761;
    hash = hash ^ (hash >> 16);
    return hash;
};

// Private helper function to get hash for a key
// This is a simplified version - in a full implementation we'd use traits/interfaces
let _get_hash = fn<K>(key: K) -> int64 {
    // This is a placeholder - the compiler will need to generate specific versions
    // based on the actual key type
    return 0;
};

// Private helper function to resize the hash map
let _resize = fn<K, V>(map: *HashMap<K, V>) -> none {
    let old_capacity = map.capacity;
    let old_entries = map.entries;
    
    // Double the capacity
    let new_capacity = old_capacity * 2;
    let entry_size = 32; // Approximate size of Entry<K, V>
    let new_entries: *Entry<K, V> = Std::Runtime.malloc(new_capacity * entry_size);
    
    // Initialize new entries
    let mut i = 0;
    unsafe {
        for i < new_capacity {
            (new_entries + i).is_occupied = false;
            i = i + 1;
        }
    }
    
    // Update map structure
    map.entries = new_entries;
    map.capacity = new_capacity;
    map.count = 0;
    
    // Rehash all existing entries
    i = 0;
    unsafe {
        for i < old_capacity {
            let entry = old_entries + i;
            if entry.is_occupied {
                // Re-insert this entry into the new table
                _insert_entry(map, entry.key, entry.value, entry.hash);
            }
            i = i + 1;
        }
    }
    
    // Free old entries
    Std::Runtime.free(old_entries);
};

// Private helper function to insert an entry (used during resize)
let _insert_entry = fn<K, V>(map: *HashMap<K, V>, key: K, value: V, hash: int64) -> none {
    let mut index = hash % map.capacity;
    if index < 0 {
        index = -index;
    }
    
    unsafe {
        // Linear probing to find an empty slot
        let mut probe_count = 0;
        for probe_count < map.capacity {
            let entry = map.entries + index;
            if !entry.is_occupied {
                // Found empty slot
                entry.key = key;
                entry.value = value;
                entry.hash = hash;
                entry.is_occupied = true;
                map.count = map.count + 1;
                return;
            }
            
            // Move to next slot (linear probing)
            index = (index + 1) % map.capacity;
            probe_count = probe_count + 1;
        }
    }
    // If we get here, the table is full - this shouldn't happen with proper resizing
};

// Creates a new HashMap with a default capacity
// Returns a pointer to a HashMap to avoid large struct copies
let new = fn<K, V>() -> *HashMap<K, V> {
    unsafe {
        let initial_capacity = 16;
        let map_size = 24; // sizeof(HashMap) - 3 * 8 bytes
        let map_ptr: *HashMap<K, V> = Std::Runtime.malloc(map_size);

        let entry_size = 32; // Approximate sizeof(Entry<K, V>)
        let entries_ptr: *Entry<K, V> = Std::Runtime.malloc(initial_capacity * entry_size);

        // Initialize entries
        let mut i = 0;
        for i < initial_capacity {
            (entries_ptr + i).is_occupied = false;
            i = i + 1;
        }

        map_ptr.entries = entries_ptr;
        map_ptr.capacity = initial_capacity;
        map_ptr.count = 0;

        return map_ptr;
    }
};

// Sets a value in the hash map
let set = fn<K, V>(map: *HashMap<K, V>, key: K, value: V) -> none {
    // Check if we need to resize (load factor > 0.7)
    if map.count * 10 > map.capacity * 7 {
        _resize(map);
    }
    
    // Get hash for the key (simplified - would need proper type dispatch)
    let hash = _get_hash(key);
    let mut index = hash % map.capacity;
    if index < 0 {
        index = -index;
    }
    
    unsafe {
        // Linear probing to find the right slot
        let mut probe_count = 0;
        for probe_count < map.capacity {
            let entry = map.entries + index;
            
            if !entry.is_occupied {
                // Found empty slot - insert new entry
                entry.key = key;
                entry.value = value;
                entry.hash = hash;
                entry.is_occupied = true;
                map.count = map.count + 1;
                return;
            }
            
            // Check if this is the same key (update existing)
            if entry.hash == hash {
                // For now, assume keys are equal if hashes match
                // In a full implementation, we'd need proper key comparison
                entry.value = value;
                return;
            }
            
            // Move to next slot (linear probing)
            index = (index + 1) % map.capacity;
            probe_count = probe_count + 1;
        }
    }
};

// Gets a value from the hash map
// For now, returns a default value on failure (0 for int64, "" for string)
// In a full implementation, this would return Option<V> or Result<V, Error>
let get = fn<K, V>(map: *HashMap<K, V>, key: K) -> V {
    let hash = _get_hash(key);
    let mut index = hash % map.capacity;
    if index < 0 {
        index = -index;
    }
    
    unsafe {
        // Linear probing to find the key
        let mut probe_count = 0;
        for probe_count < map.capacity {
            let entry = map.entries + index;
            
            if !entry.is_occupied {
                // Hit empty slot - key not found
                // Return default value (this is a limitation of the current type system)
                // In practice, we'd return None or an error
                break;
            }
            
            // Check if this is the key we're looking for
            if entry.hash == hash {
                // For now, assume keys are equal if hashes match
                return entry.value;
            }
            
            // Move to next slot (linear probing)
            index = (index + 1) % map.capacity;
            probe_count = probe_count + 1;
        }
    }
    
    // Key not found - return default value
    // This is a placeholder - in a real implementation we'd handle this properly
    let default_value: V;
    return default_value;
};

// Checks if a key exists in the hash map
let contains = fn<K, V>(map: *HashMap<K, V>, key: K) -> bool {
    let hash = _get_hash(key);
    let mut index = hash % map.capacity;
    if index < 0 {
        index = -index;
    }
    
    unsafe {
        // Linear probing to find the key
        let mut probe_count = 0;
        for probe_count < map.capacity {
            let entry = map.entries + index;
            
            if !entry.is_occupied {
                // Hit empty slot - key not found
                return false;
            }
            
            // Check if this is the key we're looking for
            if entry.hash == hash {
                // For now, assume keys are equal if hashes match
                return true;
            }
            
            // Move to next slot (linear probing)
            index = (index + 1) % map.capacity;
            probe_count = probe_count + 1;
        }
    }
    
    return false;
};

// Returns the number of key-value pairs in the hash map
let size = fn<K, V>(map: *HashMap<K, V>) -> int64 {
    return map.count;
};

// Frees the memory used by the hash map
let free = fn<K, V>(map: *HashMap<K, V>) -> none {
    unsafe {
        Std::Runtime.free(map.entries);
        Std::Runtime.free(map);
    }
}; 
