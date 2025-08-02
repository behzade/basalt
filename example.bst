import {
    Std::Os
    Std::Fmt // Modules are file based. Std::Fmt is all the code in ./modules/std/*.bst
    Std::Collections::Vector
    Self::Util // This refers to ./src/util/*.bst
}

pub Person: struct {
    name: str
    age: i32
}

// We have basically structual typing but with nominal constraints
// It can be other structs to match shape or traits to match behaviour
Person: WithAge, Printable

// self is implicitly available as the receiver
Person::get_age: () -> i32 = {
    self.age
}

Person::up_age: () -> i32 = {
    self.age += 1
    self.age
}

Printable: struct {
    name: str
}

WithAge: trait {
    get_age: () -> i32
}


Panic: effect {
    panic: (msg: str) -> !
}

OsExitPanic: Panic = {
    panic: (msg: str) -> ! = {
        Os::exit(1)
    }
}

division: (a: i32, b: i32) -> i32 with {Panic} = {
    if b == 0 {
        perform Panic::panic("division by zero")
    }
    a / b
}

Company: struct {
    company_id: i32
    name: str
}

UserType: enum {
    B2B(Company)
    B2C(Person)
}

main: () -> i32 = {
    mut person: Person = Person { name: "behzad", age: 10 }
    age: i32 = person.get_age()

    user_type: UserType = UserType::B2B(Company { company_id: 1, name: "behzad" })

    match_result : str = match user_type {
        UserType::B2B(b2b) => {
            Fmt::println("B2B")
            b2b.name
        }
        UserType::B2C(b2c) => {
            Fmt::println("B2C")
            b2c.name
        }
    }

    if_result : i32 = if age > 10 {
        3
    } else {
        4
    }

    while age > 10 {
        age -= 1
    }

    agev2: i32 = person.get_age()

    block_result: i32 = {
        a: i32= 1
        b: i32= 2
        a + b
    }

    adder : (a: i32, b: i32) -> i32 = { a + b }

    out: i32 = division(10, 0) with {OsExitPanic} // panic with exit code 1
    // if multiple effects are necessary
    // out: i32= division(10, 0) with {OsExitPanic + ConsoleLogger}

    out2: i32= division(10, 2) with {
        panic: (msg: str) -> ! = {
            Fmt::println("panic with exit code 0")
            Os::exit(0)
        }
    }
    Fmt::println(out2) // prints 5
}
