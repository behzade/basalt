// Test: Algebraic Effects with Continuation Capture
// This demonstrates how perform operations capture continuations

effect IO {
    Read() -> string,
    Write(string) -> none,
}

handler ConsoleIO{IO} {
    Read() -> string {
        return "input";
    }
    Write(msg: string) -> none {
        // console output
    }
}

fn main() -> string / {IO} {
    perform IO.Write("What's your name?");
    let name = perform IO.Read();
    return "Hello " + name;
}

fn run() -> string {
    handle main() with ConsoleIO;
} 