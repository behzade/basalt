struct Person {
    name: str,
    age: i32,
}

fn get_age(p: Person) -> i32 {
    p.age
}

fn up_age_by(mut p: Person, by: i32) -> i32 {
    p.age <- p.age + by
    p.age
}


fn main() -> i32 {
    let mut p: Person = Person { name: "behzad", age: 10 }
    let age = p.get_age()
    let inc = p.up_age_by(2)
    inc
}
