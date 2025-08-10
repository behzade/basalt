import {
    std::io
}

type Person = {
    name: str,
    age: i32,
}

type Company = {
    company_id: i32,
    name: str,
}

type bbb = asd | qwe

type UserType = B2B(Company) | B2C(Person)

fn main() -> i32 = {
    io.println("Hello, world!")
    let person: Person = { Person { name: "behzad", age: 10 } }
    let company: Company = { Company { company_id: 1, name: "acme" } }
    let user1: UserType = B2B(company)
    let user2: UserType = B2C(person)
    0
}
