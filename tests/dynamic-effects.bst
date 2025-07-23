// Test: Dynamic Effect Handler Context
// This demonstrates that effect handlers work dynamically across function calls

effect IO {
    Read() -> string,
    Write(string) -> none,
}

handler ConsoleIO{IO} {
    Read() -> string {
        return "mock_input";
    }
    Write(msg: string) -> none {
        return;
    }
}

fn greetUser() -> string / {IO} {
    perform IO.Write("What's your name?");
    let name = perform IO.Read();
    return "Hello " + name;
}

fn main() -> string {
    handle greetUser() with ConsoleIO;
} 