import Std::Runtime;

fn print_int(value: int) -> none {
    Runtime.printf("%d\n", value);
}

fn print_string(value: string) -> none {
    Runtime.printf("%s\n", value);
}

fn print_bool(value: bool) -> none {
    if value {
        Runtime.printf("true\n");
    } else {
        Runtime.printf("false\n");
    }
}

fn print_float(value: float64) -> none {
    Runtime.printf("%f\n", value);
}
