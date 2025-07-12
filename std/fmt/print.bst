import Std::Runtime;

let print_int = fn(value: int64) -> none {
    Runtime.printf("%d\n", value);
};

let print_string = fn(value: string) -> none {
    Runtime.printf("%s\n", value);
};

let print_bool = fn(value: bool) -> none {
    if value {
        Runtime.printf("true\n");
    } else {
        Runtime.printf("false\n");
    };
};

let print_float = fn(value: float64) -> none {
    Runtime.printf("%f\n", value);
};
