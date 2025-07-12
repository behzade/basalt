let user = struct {
    id: int64,
    name: string
};

let mut behzad = user {
    id: 1,
    name: "behzad"
};


for behzad.id < 10000 {
    behzad.id = behzad.id + 1;
    print(behzad.id);
}
let qwe = if false { 2 } else { 3 };

let myfun = fn(x: int64) -> int64 { x + 2};

print(myfun(3));

print(qwe);
