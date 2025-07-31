// Test 22: Effects
// Testing effects (most complex feature)

effect Print {
    fn print(msg: string) -> unit
}

handler MockPrint{Print} {
    fn print(msg: string) -> unit {
        return ()
    }
}

fn testprint() -> unit / {Print} {
    perform Print::print("Hello, World!")
}

fn main() -> i32 {
    handle {
        testprint()
    } with {
        print(msg: string) {
            return ()
        }
    }

    handle testprint() with MockPrint
    return 0
}
