let user = struct {
    id: int64,
    name: string
}

let mut behzad = user {
    id: 1,
    name: "behzad"
}


for behzad.id < 10000 {
    behzad.id = behzad.id + 1
    print(behzad.id)
}

print(behzad.id)