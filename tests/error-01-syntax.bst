// Error Test 01: Syntax Errors
// This file should fail parsing

let x = 42
// Missing semicolon

fn broken() {
    return 42
// Missing semicolon
}

let y = 1 + * 3;
// Invalid operator sequence

struct Broken {
    field: i64
// Missing semicolon
} 