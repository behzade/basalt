//! Minimal runtime library for Basalt compiled programs

use std::io::{self, Write};

/// Print a value to stdout
#[no_mangle]
pub extern "C" fn basalt_print(value: i64) {
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "{}", value);
}

/// Print a string to stdout
#[no_mangle]
pub extern "C" fn basalt_print_str(ptr: *const u8, len: usize) {
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let s = String::from_utf8_lossy(slice);
    let mut stdout = io::stdout();
    let _ = write!(stdout, "{}", s);
} 