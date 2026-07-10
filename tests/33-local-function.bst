// Named local functions retain their declaration and are callable in their scope.
fn main() -> i32 {
    fn answer() -> i32 { 41 }
    answer()
}
