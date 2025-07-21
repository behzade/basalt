// Test file with type error
// This should fail type checking

let x: i64 = "hello"; // Type error: string assigned to i64
let y = 42 + "world"; // Type error: cannot add i64 and string 