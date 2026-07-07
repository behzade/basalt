import {
    std::runtime
}

fn main() -> ! {
    runtime::write("stdout", "runtime extern")
    runtime::exit(0)
}
