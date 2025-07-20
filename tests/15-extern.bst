// Test 15: Extern
// Testing external function declarations

extern fn strlen(s: string) -> i64;
extern fn printf(format: string, ...) -> i64;
extern fn malloc(size: i64) -> rawptr;

fn useExtern() -> i64 {
    let len = strlen("hello");
    return len;
} 