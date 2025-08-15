import {
    std::io
}

struct Person {
    name: str,
    age: i32,
}

struct Company {
    company_id: i32,
    name: str,
}

type bbb = asd: unit | qwe: unit

type UserType = B2B: Company | B2C: Person

fn main() -> i32 {
    io::println("Hello, world!")
    let person: Person = { Person { name: "behzad", age: 10 } }
    let company: Company = { Company { company_id: 1, name: "acme" } }
    let user1: UserType = UserType::B2B{ company_id: 1, name: "acme" }
    let user2: UserType = UserType::B2C{ name: "behzad", age: 10 }
    0
}
