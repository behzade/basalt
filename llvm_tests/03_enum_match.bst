// Simple match test
import Std::Fmt;

let Option = enum {
    Some(int64),
    None,
};

let some_val = Option::Some(100);
let result = match some_val {
    Option::Some(x) => x + 1,
    Option::None => 0,
};

Fmt.print_int(result); 