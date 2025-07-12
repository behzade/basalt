// FILE: std/runtime.bst

// External C function declarations for the Basalt runtime.
// This syntax is compatible with the current parser.

// --- Print Functions ---
extern fn basalt_print_int(val: int64) -> none;
extern fn basalt_print_bool(val: bool) -> none;
extern fn basalt_print_string(str: string) -> none;
extern fn basalt_print_float(val: float64) -> none;

// Generic print function (implemented as a variadic function for flexibility)
extern fn printf(format: string, ...) -> none;

// --- String Functions ---
extern fn basalt_string_concat(s1: string, s2: string) -> string;
extern fn basalt_string_equals(s1: string, s2: string) -> bool;

// --- Array Functions ---
// Note: We'll need a way to represent a pointer to the array struct.
// Let's assume a type `array_ptr` that the compiler understands.
extern fn basalt_array_new(capacity: int64) -> array_ptr;
extern fn basalt_array_push(arr: array_ptr, value: int64) -> none;
extern fn basalt_array_get(arr: array_ptr, index: int64) -> int64;
extern fn basalt_array_len(arr: array_ptr) -> int64;
