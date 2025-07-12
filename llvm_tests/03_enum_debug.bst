// Debug enum test
let Option = enum {
    Some(int64),
    None,
};

let some_val = Option::Some(100);
let result = match some_val {
    Option::Some(x) => x,
    Option::None => 0,
}; 