import Std::Runtime;

// Simple test to debug struct field access through pointers
let TestStruct = struct { field: int64 };
let ptr: *TestStruct = 0;
// This should work after our fix
ptr.field = 42;
print(ptr.field); 