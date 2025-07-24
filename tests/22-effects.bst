// Test 22: Effects
// Testing effects (most complex feature)

effect Print {
    fn print(msg: string) -> unit
}

fn main() -> i32 {
    handle {
        perform Print::print("Hello, World!");
        return 0;
    } with {
        print(msg) {
            // In a real implementation, this would print to stdout
            return ();
        }
    }
} 