// file based modules, each directory is a module, self refers to src directory, other modules are in ./modules
import {
    std::runtime
    std::fmt
    std::fmt as second_fmt // aliasing. if imports have conflicts, compiler forces you to use an alias
    std::collections::vector
    self::util
}

// Language targets llvm ir, variables will immutable by default.

struct Person {
    name: str,
    age: i32,
}

struct Company { 
    company_id: i32,
    name: str
}

type ExampleTaggedEnum = TaggedUnit: unit | TaggedI32: i32
type ExampleUntaggedEnum = unit | i32

type UserType = B2B: Company | B2C: Person

let mut p2 = Person { name: "behzad", age: 10 }

effect Panic {
    panic(msg: str) -> !
}

handler OsExitPanic: Panic {
    fn panic(msg: str) -> ! {
        runtime::exit(1)
    }
}

fn division(a: i32, b: i32) -> i32 effects { Panic } {
    if b == 0 {
        perform Panic.panic("division by zero")
    }
    a / b
}

fn get_age(person: Person) -> i32 { person.age }

fn main() -> i32 {
    // does not allow shorthand omitted field names because adding fields to structs is a breaking change
    let mut person = Person { name: "behzad", age: 10 }
    let age: i32 = person.get_age() // UFCS

    let user_type = UserType::B2B{ company_id: 1, name: "behzad"}

    let match_result = match user_type
        | b2b: B2B -> b2b.name
        | b2c: B2C -> b2c.name
        | _ -> "unknown"

    let if_result = if age > 10 {
        3
    } else {
        4
    }

    while age > 10 {
        age = age - 1
    }

    let block_result: i32 = {
        a: i32= 1
        b: i32= 2
        a + b
    }

    fn adder(a: i32, b: i32) -> i32 { a + b }

    let out: i32 = division(10, 0) with {OsExitPanic} // panic with exit code 1

    let out2 = division(10, 2) with {OsExitPanic}
    fmt::println(out2) // prints 5
    0
}


// compile time function execution
let a = meta {
    if runtime.is_32bit() {
        4
    } else {
        8
    }
}

// embed file in binary in compile time
let configFile = meta std.io.read_file("config.json") // example syntax

// -- Effect Definitions --
// Effects declare operations that are supplied by handlers in the active context.

// The Http effect declares an operation for getting web content.
effect Http {
    get(url: str) -> str
}

// The Async effect declares an operation to run a task and await its result.
// This allows a computation to be suspended and resumed later.
effect Async<T> {
    await(task: () -> T) -> T
}


// HttpHandler implements the 'Http' effect.
// It is declared 'effects {Async}', meaning its implementation can, in turn,
// perform 'Async' effects.
handler HttpHandler: Http effects {Async<str>} {
    // This provides the implementation for the 'get' operation.
    fn get(url: str) -> str effects {Async<str>} {
        // We define the actual work as a lambda function.
        // This function captures the 'url' from its environment.
        fn http_task() -> str {
            // In a real-world scenario, this would be a blocking network call.
            // For this example, we'll simulate it by returning a string.
            // We can imagine this is where a library like 'requests.get()' would be called.
            return "<html><body><h1>Content for " + url + "</h1></body></html>"
        }

        // Instead of executing the task directly, we 'perform' the 'await' effect.
        // We hand the task over to the Async handler (the Scheduler).
        // The handler will execute the task and resume this function with the result.
        let content = perform Async.await(http_task)
        return content
    }
}

// The Scheduler handler provides the implementation for the 'Async' effect.
// It controls how asynchronous tasks are executed.
handler Scheduler: Async<T> {
    // This provides the implementation for the 'await' operation.
    // The rule "return resumes the calling function" means the value returned
    // by this handler function will be sent back to the site of the 'perform' call.
    fn await(task: () -> T) -> T {
        // A real scheduler would manage an event loop or a thread pool to run
        // tasks concurrently. It would suspend the current computation, run the
        // task in the background, and resume the computation only when the
        // task is complete.

        // For this simplified example, we execute the task immediately.
        let result = task()

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
fn fetch_content_size(url: str) -> i32 effects {Http} {
    // 'perform' invokes an effect operation. Control is transferred to the
    // currently active handler for the 'Http' effect.
    let content = perform Http.get(url)
    return content.len()
}


with { Handler1, Handler2 } {
     // Code inside this block will use Handler1 and Handler2 as the
     // default handlers for their respective effects.
     // the with {handler, handler2} can be omitted
}

// Memory management

// The language has a concept of tiered contexts. The root context is passed to the main function.
// Each context has a table of handlers available for scope and an arena memory allocator.
// Each function call or a scope "with" block creates a child context.
// In compile type, the compiler will convert the implicit context into a struct that has the handler table and arena and implicitly pass it to the functions.
// On function calls, the paramters and return value are allocated in the parent context's arena. This allows sending data outside the function. Function can not send pointers to its local data to the outside.
