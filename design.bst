import {
    std/os
    // Modules are file based. all the code in ./modules/std/*.bst
    std/fmt
    std/collections/vector
    // This refers to ./src/util/*.bst
    self/util
}

// Language targets wasm, local variables will immutable by default, and stored in linear memory, escaping variables will be stored in heap using wasm gc proposal

// We have basically structual typing but with nominal constraints
// It can be other structs to match shape or interfaces to match behaviour
pub Person: struct {
    name: str
    age: i32
}: WithAge

// methods are just functions, no explicit implementation for interfaces, if required functions are not available, the struct definition errors
get_age: (p: Person) -> i32 = {p.age}

// you can group function by partial application
// | binds the variables to the scope (kinda similar to unix pipes)
(p: Person)|{
    get_age: () -> i32 = {p.age}
    up_age: () -> i32 = {p.age += 1; p.age}
    up_age_by: (n: i32) -> i32 = {p.age += n; p.age}
}

(p: Person, n: i32)|{
    up_age_by: () -> i32 = {p.age += n; p.age}
    down_age_by: () -> i32 = {p.age -= n; p.age}
}

// you can do partial application like this
p2: Person = Person { name: "behzad", age: 10 }

up_age_by_behzad := p2|up_age_by

up_age_by_behzad_10 := (p2, 10)|up_age_by
// or named arguments
up_age_by_behzad_10 := (n: 10, p: p2)|up_age_by

WithAge: interface {
    get_age: () -> i32
}

Panic: effect {
    panic: (msg: str) -> !
}

OsExitPanic: Panic = {
    panic: (msg: str) -> ! = {
        os.exit(1)
    }
}

division: (a: i32, b: i32) -> i32 with {Panic} = {
    if b == 0 {
        perform Panic.panic("division by zero")
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
    mut person: Person = { name: "behzad", age: 10 }
    age: i32 = person.get_age()

    age2: i32 = get_age(person) // also works: Univeral Function Calls

    user_type: UserType = UserType.B2B(Company { company_id: 1, name: "behzad" })

    match_result: str = match user_type {
        UserType.B2B(b2b) -> {
            fmt.println("B2B")
            b2b.name
        }
        UserType.B2C(b2c) -> {
            fmt.println("B2C")
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
    // out: i32= division(10, 0) with {OsExitPanic,ConsoleLogger}
    // i don't know if overlapping functions should be allowed by handlers of effects

    // type annotation between : and = can be omitted for type inference
    out2 := division(10, 2) with {
        panic: (msg: str) -> ! = {
            fmt.println("panic with exit code 0")
            os.exit(0)
        }
    }
    fmt.println(out2) // prints 5
}


// compile time function execution
a := meta {
    if runtime.is_wasm() {
        4
    } else {
        8
    }
}

Random: effect {
    rand: () -> i32
}

a : () with {Random} -> i64 = { rnd := perform Random.rand(); rnd / 10 }

let b := a() with {
    rand: () -> i32 = {
        return 10 // return is just resume. language only allows single continuation for effects
    }
}


// -- Effect Definitions --
// Effects are like interfaces that declare a set of operations.

// The Http effect declares an operation for getting web content.
effect Http {
    get: (url: str) -> str
}

// The Async effect declares an operation to run a task and await its result.
// This allows a computation to be suspended and resumed later.
effect Async<T> {
    await: (task: () -> T) -> T
}


// -- Handler Implementations --
// Handlers provide the concrete implementation for effect operations.

// HttpHandler implements the 'Http' effect.
// It is declared 'with {Async}', meaning its implementation can, in turn,
// perform 'Async' effects.
handler HttpHandler: Http with {Async<str>} = {
    // This provides the implementation for the 'get' operation.
    get: (url: str) -> str with {Async<str>} = {
        // We define the actual work as a lambda function.
        // This function captures the 'url' from its environment.
        http_task := () => {
            // In a real-world scenario, this would be a blocking network call.
            // For this example, we'll simulate it by returning a string.
            // We can imagine this is where a library like 'requests.get()' would be called.
            return "<html><body><h1>Content for " + url + "</h1></body></html>"
        }

        // Instead of executing the task directly, we 'perform' the 'await' effect.
        // We hand the task over to the Async handler (the Scheduler).
        // The handler will execute the task and resume this function with the result.
        content := perform Async.await(http_task)
        return content
    }
}

// The Scheduler handler provides the implementation for the 'Async' effect.
// It controls how asynchronous tasks are executed.
handler Scheduler<T>: Async<T> = {
    // This provides the implementation for the 'await' operation.
    // The rule "return resumes the calling function" means the value returned
    // by this handler function will be sent back to the site of the 'perform' call.
    await: (task: () -> T) -> T = {
        // A real scheduler would manage an event loop or a thread pool to run
        // tasks concurrently. It would suspend the current computation, run the
        // task in the background, and resume the computation only when the
        // task is complete.

        // For this simplified example, we execute the task immediately.
        result := task()

        // Returning the result here resumes the suspended 'HttpHandler.get' function,
        // providing the value of 'content'.
        return result
    }
}

// -- Handler Composition --
// We compose handlers to create a full execution environment.
// 'AsyncHttpHandler' handles 'Http' effects using 'HttpHandler', and any 'Async'
// effects performed by 'HttpHandler' are then handled by the 'Scheduler'.
let AsyncHttpHandler = HttpHandler with {Scheduler}


// -- Main Program Logic --
// This function uses the 'Http' effect but is unaware of the async implementation.
fetch_content_size: (url: str) -> i32 with {Http} = {
    // 'perform' invokes an effect operation. Control is transferred to the
    // currently active handler for the 'Http' effect.
    content := perform Http.get(url)
    return content.len()
}


// -- Execution --
// We run the main logic, providing the 'AsyncHttpHandler' to handle the effects.
fn main() -> i32 {
    // The 'with {AsyncHttpHandler}' clause provides the full implementation
    // for all effects that 'fetch_content_size' might perform.
    size := fetch_content_size("https://www.google.com") with {AsyncHttpHandler}

    0
}
