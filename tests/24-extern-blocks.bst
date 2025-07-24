// Test 24: Extern blocks
// Testing extern function declarations

extern "C" {
    fn printf(format: *const u8, ...) -> i32;
    fn strlen(s: *const u8) -> usize;
}

fn main() -> i32 {
    // In a real implementation, we would call the extern functions
    // For now, just return success
    return 0;
} 