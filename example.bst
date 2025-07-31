import {
    Std::Os
    Std::Fmt
    Std::Collections::Vector
}

// this is both so that structual typing does not lead to accidentally implementing a trait we don't want to and to simplify the compiler work of finding all the
// instances of a trait. The struct type definition, the functions that get turned into methods and the satisfies clause should be all in the same module/namespace
struct Person {
    name: String,
    age: u32,
}

Person satisfies WithAge, Printable

// after defining this, p.get_age() is available. this does not happen globally. the fucntion and the method must be in the same module/namespace
fn get_age(p: Person) -> i32 {
    p.age
}

// the language will be garbage collected. mut is not about ownership. it's about mutability. the language will not allow you to mutate a value that is not mutable
fn up_age(p: mut Person) -> i32 {
    p.age += 1
    p.age
}

struct Printable {
    name: String,
}

trait WithAge {
    get_age(self) -> i32
}

// in another module I want person to satisfy my new trait.

trait WithMyAge {
    get_age(self) -> i32
}

Person satisfies WithMyAge

// I want to satisfy a trait with functions that have different names

trait NewWithAge {
    get_age_3(self) -> i32
}

Person satisfies NewWithAge {
    get_age_3(self) -> i32 {
        self.age
    }
}

effect Panic {
    panic(msg: String) -> !
}

handler OsExitPanic{Panic} {
    fn panic(msg: String) -> ! {
        Os::exit(1)
    }
}

fn division(a: i32, b: i32) -> i32 / {Panic} {
    if b == 0 {
        perform Panic::panic("division by zero")
    }
    a / b
}

enum UserType {
    B2B(company_id: u32, user_id: u32),
    B2C(user_id: u32),
}

fn main() {
    let mut person = Person { name: "behzad", age: 10 }
    let age = get_age(person)

    let user_type = UserType::B2B({company_id: 1, user_id: 2})

    let match_result = match user_type {
        UserType::B2B(b2b) => {
            Fmt::println("B2B")
            b2b.company_id
        }
        UserType::B2C(b2c) => {
            Fmt::println("B2C")
            b2c.user_id
        }
    }

    let if_result = if age > 10 {
        3
    } else {
        4
    }

    while age > 10 {
        age -= 1
    }

    let agev2 = person.get_age()

    let block_result = {
        let a = 1
        let b = 2
        a + b
    }

    person
       |> up_age // pipes the output of previous function as the first argument of the next function
       |> Fmt::println

    let adder = fn(a: i32, b: i32) -> i32 { a + b }

    person.age
        |> adder(2) // can pass partial arguments
        |> Fmt::println

    person
        |> up_age
        |> ({age: i32}) -> () { Fmt::println(age + 2) } // anonymous function

    let out = handle division(10, 0) with OsExitPanic

    handle division(10, 2) with {
        panic(msg: String) -> ! {
            Fmt::println("panic with exit code 0")
            Os::exit(0)
        }
    } |> Fmt::println // prints 5

}
