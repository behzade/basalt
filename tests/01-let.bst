let b: i64 = 2;
let c: f64 = 3.0;
let d: bool = true;
let e: string = "hello";
let f: Array<i64> = [1, 2, 3];
let g: Array<Array<i64>> = [[1, 2, 3], [4, 5, 6]];
let h: Map<string, i64> = {"a": 1, "b": 2};

struct s {
    a: i64,
    b: f64,
};

let instance = s {
    a: 1,
    b: 2.0,
};

trait tr {
    add(a: i64, b: f64) -> f64;
};

impl tr for s {
    add(a: i64, b: f64) -> f64 {
        return a + b;
    }
}

struct genericStruct<T,F> {
    a: T,
    b: F,
};

let genericInstance = genericStruct<i64, f64> {
    a: 1,
    b: 2.0,
};


import Std::Collections::Map;

let testMap = Map<string, i64> {
    "a": 1,
    "b": 2,
    "c": 3,
};

let testMap2 = Std::Collections::Map<string, i64> {
    "a": 1,
    "b": 2,
    "c": 3,
};

// import is just syntax sugar for a namespace
import Std::Collections::Map as MyMap;
MyMap::New();

// should get parsed to
Std::Collections::Map::New();

let mut counter = 0;

while counter < 10 {
    counter = counter + 1;
};

// comments
fn basicFn(a: i64, b: f64) -> f64 {
    return a + b;
};

effect eff {
    Write(string) -> none,
};

handler effHandler{eff} {
    Write(input:string) -> none {
        printf("writing %s\n", input);
    }
};

fn myFn(input: string) -> string / {eff} {
    perform eff.Write(input);
    return input;
};

let handleResult = handle myFn with effHandler;

let handleResult2 = handle myFn with {
    Write(input:string) -> none {
        printf("new writing %s\n", input);
    }
};

// external functions linked at compile time
extern fn strlen(s: string) -> int64;

let expressionResult = {
    let a = 1;
    let b = 2;
    a + b
};

let myEnum = enum {
    A,
    B(i64),
    C(f64),
};

let myEnumInstance = myEnum::A;

let matchResult = match myEnumInstance {
    A => 1,
    B(a) => a,
    C(a) => a,
};

let ifResult = if true {
    1
} else {
    2
};
