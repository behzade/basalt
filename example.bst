import {
    Std::Os
    Std::Fmt
    Std::Collections::Vector
}

Person: struct {
    name: String,
    age: u32,
}

Person: WithAge, Printable

Person::get_age: (self: Self) -> i32 = {
    self.age
}

Person::up_age: (self: mut Self) -> i32 = {
    self.age += 1
    self.age
}

Printable: struct {
    name: String,
}

WithAge: trait {
    get_age: (self) -> i32
}

// in another module I want person to satisfy my new trait.

WithMyAge: trait {
    get_age: (self) -> i32
}

Person: WithMyAge

// I want to satisfy a trait with functions that have different names

NewWithAge: trait {
    get_age_3 : (self) -> i32
}

Person: NewWithAge {
    get_age_3: (self) -> i32 = {
        self.age
    }
}

Panic: effect {
    panic: (msg: String) -> !
}

OsExitPanic: Panic = {
    panic: (msg: String) -> ! = {
        Os::exit(1)
    }
}

division: (a: i32, b: i32) -> i32 / {Panic} = {
    if b == 0 {
        perform Panic::panic("division by zero")
    }
    a / b
}

UserType: enum {
    B2B(company_id: u32, user_id: u32),
    B2C(user_id: u32),
}

main: () -> i32 = {
    mut person := Person { name: "behzad", age: 10 }
    age := get_age(person)

    user_type := UserType::B2B{company_id: 1, user_id: 2}

    match_result := match user_type {
        UserType::B2B(b2b) => {
            Fmt::println("B2B")
            b2b.company_id
        }
        UserType::B2C(b2c) => {
            Fmt::println("B2C")
            b2c.user_id
        }
    }

    if_result := if age > 10 {
        3
    } else {
        4
    }

    while age > 10 {
        age -= 1
    }

    agev2 := person.get_age()

    block_result := {
        a := 1
        b := 2
        a + b
    }

    person
       |> up_age // pipes the output of previous function as the first argument of the next function
       |> Fmt::println

    adder : (a: i32, b: i32) -> i32 = { a + b }

    person.age
        |> adder(2) // can pass partial arguments
        |> Fmt::println

    out := division(10, 0) with {OsExitPanic}

    division(10, 2) with {
        panic: (msg: String) -> ! = {
            Fmt::println("panic with exit code 0")
            Os::exit(0)
        }
    } |> Fmt::println // prints 5
}
