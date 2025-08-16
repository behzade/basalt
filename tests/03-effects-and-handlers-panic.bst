import {
    std::os
}


effect Panic {
    panic(msg: str) -> !
}

handler OsExitPanic: Panic {
    fn panic(msg: str) -> ! {
        os::exit(1);
    }
}

fn division(a: i32, b: i32) -> i32 with Panic {
    if b == 0 {
        perform Panic.panic("division by zero")
    }
    a / b
}

fn main() -> i32 {
    let _ = division(10, 2) with {OsExitPanic}
    0
}
