
pub fn exit(code: i32) -> ! {
    asm!("syscall")
}
