// Test 14: Effects
// Testing effect definitions and handlers

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

fn greetUser() -> string / {IO} {
    perform IO.Write("What's your name?");
    let name = perform IO.Read();
    return "Hello " + name;
} 