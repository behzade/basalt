// file based modules, each directory is a module, self refers to src directory, other modules are in ./modules
import {
    std::os
    std::fmt
    std::fmt as second_fmt // aliasing. if imports have conflicts, compiler forces you to use an alias
    std::collections::vector
    self::util
}

// Language targets cranelift ir, variables will immutable by default.

type Person = {
    name: str,
    age: i32,
}
type Company = { 
    company_id: i32,
    name: str
}
type UserType = B2B(Company) | B2C(Person)

interface WithAge = {
    get_age(self) -> i32
    up_age_by(mut self, age: i32) -> i32 
}

impl Person = {
    fn new(name: str, age: i32) -> Person = Person { name: name, age: age }
}
impl Person: WithAge = {
    fn get_age(self) -> i32 = { self.age }
    fn up_age_by(mut self, n: i32) -> i32 = { self.age <- self.age + n; self.age }
}

let mut p2: Person = Person { name: "behzad", age: 10 }

// currying
let up_age_by_behzad = up_age_by(p2) // if p2 was not mutable, this would be a compiler error

effect Panic = {
    panic(msg: str) -> !
}

handler OsExitPanic: Panic = {
    fn panic(msg: str) -> ! = {
        os.exit(1)
    }
}

fn division(a: i32, b: i32) -> i32 with Panic = {
    if b == 0 {
        perform Panic.panic("division by zero")
    }
    a / b
}

fn main() -> i32 = {
    // does not allow shorthand omitted field names because adding fields to structs is a breaking change
    let mut person: Person = Person { name: "behzad", age: 10 }
    let age: i32 = person.get_age()

    let user_type = B2B(Company { company_id: 1, name: "behzad" })

    let match_result = match user_type
        | B2B(b2b) -> b2b.name
        | B2C(b2c) -> b2c.name
        | _ -> "unknown"

    let if_result = if age > 10 {
        3
    } else {
        4
    }

    while age > 10 {
        age <- age - 1
    }

    let block_result: i32 = {
        a: i32= 1
        b: i32= 2
        a + b
    }

    fn adder(a: i32, b: i32) -> i32 = { a + b }

    let out: i32 = division(10, 0) with {OsExitPanic} // panic with exit code 1

    let out2 = division(10, 2) with {OsExitPanic}
    fmt.println(out2) // prints 5
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
// Effects are like interfaces that declare a set of operations.

// The Http effect declares an operation for getting web content.
effect Http = {
    get(url: str) -> str
}

// The Async effect declares an operation to run a task and await its result.
// This allows a computation to be suspended and resumed later.
effect Async<T> = {
    await(task: () -> T) -> T
}


// -- Handler Implementations --
// Handlers provide the concrete implementation for effect operations.

// HttpHandler implements the 'Http' effect.
// It is declared 'with {Async}', meaning its implementation can, in turn,
// perform 'Async' effects.
handler HttpHandler: Http with Async<str> {
    // This provides the implementation for the 'get' operation.
    fn get(url: str) -> str with {Async<str>} = {
        // We define the actual work as a lambda function.
        // This function captures the 'url' from its environment.
        fn http_task() -> str = {
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
    fn await(task: () -> T) -> T = {
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
fn fetch_content_size(url: str) -> i32 with {Http} = {
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
