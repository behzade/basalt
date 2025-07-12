// Complete enum test
import Std::Fmt;

let Option = enum {
    Some(int64),
    None,
};

let some_val = Option::Some(100);
let none_val = Option::None;

let result1 = match some_val {
    Option::Some(x) => x + 1,
    Option::None => 0,
};

let result2 = match none_val {
    Option::Some(x) => x,
    Option::None => -1,
};

Fmt.print_int(result1);
Fmt.print_int(result2); 