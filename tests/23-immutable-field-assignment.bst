struct Person {
    name: str,
    age: i32,
}

fn main() -> i32 {
    let person: Person = Person { name: "behzad", age: 10 }
    person.age = 11
    person.age
}
