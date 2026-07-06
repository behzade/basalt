fn main() -> i32 {
    let i8_value: i8 = 1i8
    let i16_value: i16 = 2i16
    let i32_value: i32 = 3
    let i64_value: i64 = 4i64

    let u8_value: u8 = 5u8
    let u16_value: u16 = 6u16
    let u32_value: u32 = 7u32
    let u64_value: u64 = 8u64

    let f32_value: f32 = 1.5f32
    let f64_value: f64 = 2.5

    let widened_signed: i64 = i8_value + i64_value
    let widened_float: f64 = f32_value + f64_value

    i32_value
}
