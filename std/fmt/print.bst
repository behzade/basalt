import Std::Runtime;

let print_int = fn(value: int64) -> none {
    let _ = Runtime.printf("%d\n", value);
};

let print_string = fn(value: string) -> none {
    let _ = Runtime.printf("%s\n", value);
};

let print_bool = fn(value: bool) -> none {
    if value {
        let _ = Runtime.printf("true\n");
    } else {
        let _ = Runtime.printf("false\n");
    };
};

let print_float = fn(value: float64) -> none {
    let _ = Runtime.printf("%f\n", value);
};
