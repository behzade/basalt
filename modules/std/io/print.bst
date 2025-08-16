// Std I/O helpers built on top of std::runtime::write
import {
    std::runtime
}

pub fn print(data: str) { runtime::write("stdout", data) }
