interface WithAge = {
    get_age(self) -> i32
    up_age_by(mut self, age: i32) -> i32
}

type Person = {
    name: str,
    age: i32,
}

impl Person = {
    fn new(name: str, age: i32) -> Person = { { name: name, age: age } }
}

impl Person: WithAge = {
    fn get_age(self) -> i32 = { self.age }
    fn up_age_by(mut self, n: i32) -> i32 = { self.age <- self.age + n; self.age }
}

fn main() -> i32 = {
    let mut p: Person = { name: "behzad", age: 10 }
    let age = p.get_age()
    let inc = p.up_age_by(2)
    inc
}