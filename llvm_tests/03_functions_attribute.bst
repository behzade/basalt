import Std::Fmt;

#[nogc]
let with_attributes = fn(x: int64) -> int64 {
    x + 1
};

Fmt.print_int(with_attributes(5));
