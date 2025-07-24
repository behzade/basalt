// Test 08: Basic functions
// Testing multiple function definitions

fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn greet(name: string) -> string {
    return "Hello " + name;
}

fn noReturn() {
    let x = 42;
}

fn main() -> i32 {
    let sum = add(10, 20);
    let greeting = greet("World");
    noReturn();
    return 0;
} 