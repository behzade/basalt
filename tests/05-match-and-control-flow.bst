type Person = { name: str, age: i32 }

type Company = { company_id: i32, name: str }

type UserType = B2B(Company) | B2C(Person)

fn main() -> i32 = {
    let mut person: Person = { name: "behzad", age: 12 }
    let user: UserType = B2B({ company_id: 1, name: "acme" })

    let match_result = match user
        | B2B(b2b) -> b2b.name
        | B2C(b2c) -> b2c.name
        | _ -> "unknown"

    let if_result = if person.age > 10 { 3 } else { 4 }

    while person.age > 10 {
        person.age <- person.age - 1
    }

    let block_result: i32 = {
        a: i32= 1
        b: i32= 2
        a + b
    }

    if_result + block_result + match_result.len()
}