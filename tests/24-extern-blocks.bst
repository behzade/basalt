// Test 24: Extern blocks
// Testing extern function declarations with extern blocks

extern "wasi" {
    fn fd_write(fd: i32, data: string) -> i32
    fn random_get(buf: [i32]) -> i32
}

struct IoVec {
    ptr: i32,
    len: i32,
}

fn main() -> i32 {
    // In a real implementation, we would call the extern functions
    // For now, just return success
    return 0
} 
