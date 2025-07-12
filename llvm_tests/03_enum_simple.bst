// Simple enum test
import Std::Fmt;

let Option = enum {
    Some(int64),
    None,
};

let some_val = Option::Some(100);
Fmt.print_int(some_val); 
