// file based modules, each directory is a module, self refers to src directory, other modules are in ./modules
import {
    std/os
    std/fmt
    std/collections/vector
    self/util
}

// Language targets cranelift ir, variables will immutable by default, local variable will be stored in stack, escaping variables will be handled by a ARC GC

pub Person: struct {
    name: str,
    age: i32,
}

WithAge: interface {
    get_age: (self) -> i32
    up_age_by: (age: i32) -> i32
}

Person: impl WithAge {
    get_age: (self) -> i32 {
        self.age
    }
    up_age_by: (self, age: i32) -> i32 {
        self.age += age
        return self.age
    }
}

Person: impl {
    new: (name: str, age: i32) -> Person {
        Person { name: name, age: age }
    }
}

// you can do partial application like this
p2: Person = Person { name: "behzad", age: 10 }

up_age_by_behzad := up_age_by(p2)
// this results in a function that only has an age argument

Panic: effect {
    panic: (msg: str) -> !
}

OsExitPanic: handler Panic {
    panic: (msg: str) -> ! {
        os.exit(1)
    }
}

division: (a: i32, b: i32) -> i32 with Panic {
    if b == 0 {
        perform Panic.panic("division by zero")
    }
    a / b
}

Company: struct {
    company_id: i32,
    name: str,
}

UserType: enum {
    B2B(Company)
    B2C(Person)
}

main: () -> i32 {
    mut person: Person = { name: "behzad", age: 10 }
    age: i32 = person.get_age()

    age2: i32 = get_age(person) // also works: Univeral Function Calls

    user_type: UserType = UserType::B2B(Company { company_id: 1, name: "behzad" })

    match_result: str = match user_type {
        UserType::B2B(b2b) => {
            fmt.println("B2B")
            b2b.name
        }
        UserType::B2C(b2c) => {
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

    adder : (a: i32, b: i32) -> i32 { a + b }

    out: i32 = division(10, 0) with {OsExitPanic} // panic with exit code 1
    // if multiple effects are necessary
    // out: i32= division(10, 0) with {OsExitPanic,ConsoleLogger}
    // i don't know if overlapping functions should be allowed by handlers of effects

    // type annotation between : and = can be omitted for type inference
    out2 := division(10, 2) with {
        panic: (msg: str) -> ! {
            fmt.println("panic with exit code 0")
            os.exit(0)
        }
    }
    fmt.println(out2) // prints 5
}


// compile time function execution
a := meta {
    if runtime.is_32bit() {
        4
    } else {
        8
    }
}

// embed file in binary in compile time
configFile := meta std.io.read_file("config.json") // example syntax

// -- Effect Definitions --
// Effects are like interfaces that declare a set of operations.

// The Http effect declares an operation for getting web content.
Http: effect {
    get: (url: str) -> str
}

// The Async effect declares an operation to run a task and await its result.
// This allows a computation to be suspended and resumed later.
Async: effect<T> {
    await: (task: () -> T) -> T
}


// -- Handler Implementations --
// Handlers provide the concrete implementation for effect operations.

// HttpHandler implements the 'Http' effect.
// It is declared 'with {Async}', meaning its implementation can, in turn,
// perform 'Async' effects.
HttpHandler: handler Http with {Async<str>} {
    // This provides the implementation for the 'get' operation.
    get: (url: str) -> str with {Async<str>} {
        // We define the actual work as a lambda function.
        // This function captures the 'url' from its environment.
        http_task : () -> str {
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
Scheduler: handler Async<T> {
    // This provides the implementation for the 'await' operation.
    // The rule "return resumes the calling function" means the value returned
    // by this handler function will be sent back to the site of the 'perform' call.
    await: (task: () -> T) -> T {
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
AsyncHttpHandler := HttpHandler with {Scheduler}


// -- Main Program Logic --
// This function uses the 'Http' effect but is unaware of the async implementation.
fetch_content_size: (url: str) -> i32 with {Http} {
    // 'perform' invokes an effect operation. Control is transferred to the
    // currently active handler for the 'Http' effect.
    content := perform Http.get(url)
    return content.len()
}


// --- Effect Handling Extensions ---
// To improve productivity and reduce boilerplate, the language supports
// default and scoped effect handlers. This allows for clean call sites
// without sacrificing explicitness or safety.

// -- Default Effect Handlers --
// The standard library provides a global effect registry. At application
// startup, a program can register default handlers for any effect.

// Example: Registering a default panic handler in main.
std.effect.register(Panic, OsExitPanic)

// Once a default is registered, any function call that performs the
// effect will automatically use this handler, unless overridden by a
// more specific handler (see below). If an effect is performed with
// no registered default and no explicit handler, it is a compile-time error.


// -- Scoped Effect Handlers --
// For more granular control, a `with` block can be used to define a
// lexical scope where a specific set of handlers are active. These
// scoped handlers take precedence over any globally registered handlers.

// Syntax:
with { Handler1, Handler2, ... } {
     // Code inside this block will use Handler1 and Handler2 as the
     // default handlers for their respective effects.
}

// This is ideal for managing contextual configurations, such as providing
// a specific database handler to a repository within a service layer.


// -- Handler Resolution Order --
// The compiler uses a strict hierarchical rule to find a handler for an
// effect performed at a call site. It searches in the following order:
//
// 1.  **Explicit Handler**: An inline `with { ... }` block on the function call.
// 2.  **Innermost Scoped Handler**: The handler from the nearest surrounding `with { ... }` block.
// 3.  **Outer Scoped Handlers**: Continues searching outwards through any nested `with` blocks.
// 4.  **Global Registered Handler**: The application-wide default from the effect registry.
// 5.  **Compile-Time Error**: If no handler is found at any level.
