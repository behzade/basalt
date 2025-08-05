# **Basalt Language Specification**

## **1. Philosophy**

**Basalt** is a statically-typed, expression-oriented language designed for safety, developer productivity, and portability by targeting WebAssembly (WASM). It achieves safety through a robust type system and a typed algebraic effects system for handling side effects. It aims for developer productivity through clear, concise syntax and powerful abstractions.

---
## **2. Syntax and Variables**

Basalt is expression-oriented. Most constructs, including `if`, `match`, and blocks, are expressions that evaluate to a value.

### **Declarations**

Variables are immutable by default. The `mut` keyword is used to declare a mutable binding. Type annotations are required, but can be omitted when the type can be inferred by the compiler using the `:=` operator.

```
// Immutable binding with explicit type
p: Person = Person { name: "behzad", age: 10 }

// Mutable binding with explicit type
mut person: Person = { name: "behzad", age: 10 }

// Type inference
inferred := "some value"
```

### **Definite Assignment**

Basalt enforces **definite assignment analysis**. It is a **compile-time error** to read from a variable before the compiler can prove it has been assigned a value on every possible code path. There are no default `null` or zeroed values.

---
## **3. Types and Data Structures**

Basalt uses a structural typing system, where type compatibility is determined by shape, but allows for nominal constraints through interfaces.

### **Primitives**

* `i8, i16, i32, i64`: Signed integers.
* `u8, u16, u32, u64`: Unsigned integers.
* `f32, f64`: Floating-point numbers.
* `bool`: `true` or `false`.
* `str`: A UTF-8 encoded string type.
* `()`: The "unit" type, representing no value.
* `!`: The "never" type. It indicates that a function will never return control, for example by panicking or exiting the process.

### **Composite Types**

**Structs** define a product type. Fields are private by default and can be exposed with the `pub` keyword.

```
pub Person: struct {
    name: str,
    age: i32,
}
```

**Enums** define a sum type (tagged union), where each variant can hold associated data.

```
UserType: enum {
    B2B(Company),
    B2C(Person),
}
```

### **Type Aliases**

The `type` keyword can create a new name for an existing type to improve readability.

```
type UserID = i32
```

---
## **4. Memory Model: WASM Target**

Basalt is designed to compile efficiently to WebAssembly.
* **Local Variables**: Non-escaping variables, including structs and other local data, are stored in **WASM linear memory** for maximum performance.
* **Heap Allocation**: Variables that escape their local scope are allocated on the heap, managed by the **WASM Garbage Collection (GC)** proposal. This avoids the complexities of manual memory management or a runtime borrow checker.

---
## **5. Behavior and Abstraction**

Behavior is defined and implemented using `interface` and `impl` blocks.

### **`interface`: Defining Shared Behavior**

An **`interface`** defines a contract—a set of function signatures that a type must implement to conform to the interface.

```
WithAge: interface {
    get_age: () -> i32
    up_age_by: (age: i32) -> i32
}
```

### **`impl`: Providing Implementations**

An **`impl`** block is used to implement an interface for a type or to define methods directly on a type.

```
// Implement the WithAge interface for the Person struct
Person: impl WithAge {
    get_age: (self) -> i32 {
        self.age
    }
    up_age_by: (self, age: i32) -> i32 {
        self.age += age
        return self.age
    }
}

// Define associated functions (static methods)
Person: impl {
    new: (name: str, age: i32) -> Person {
        Person { name, age }
    }
}
```

---
## **6. Functions & Control Flow**

Functions are first-class citizens. They can be passed as arguments, returned from other functions, and support partial application.

```
// Standard function definition
adder: (a: i32, b: i32) -> i32 { a + b }

// Partial application
up_age_by_behzad := up_age_by(p2) // p2 is a Person
// up_age_by_behzad is now a function: (age: i32) -> i32
```

### **Universal Function Call (UFC)**

Functions can be called with method-like syntax if the first argument's type matches, improving readability.

```
age := person.get_age() // Method call
age2 := get_age(person) // Also works via UFC
```

### **Control Flow Expressions**

`if`/`else` and `match` are expressions that must be exhaustive and evaluate to a value of a consistent type across all branches.

```
// if expression
result := if age > 10 { 3 } else { 4 }

// match expression
name := match user_type {
    UserType::B2B(b) => b.name,
    UserType::B2C(c) => c.name,
}
```

---
## **7. Modularity and Visibility**

* **Modules**: Each directory is a module. The `import` keyword brings other modules into scope. `std` refers to the standard library, `self` refers to the current module's root directory, and `./` refers to a relative path.
* **Visibility**: All items (`fn`, `struct`, `enum`, etc.) and struct fields are **private by default**. The `pub` keyword makes an item visible outside its module.

```
import {
    std/os
    std/fmt
    self/util
}
```

---
## **8. Error Handling & Effects**

Basalt uses a typed algebraic effects system for managing side effects like I/O, errors, and asynchrony. This separates what a function does from how it is done.

### **`effect` and `perform`**

An **`effect`** defines a set of operations. A function that uses an effect must declare it in its signature. The `perform` keyword invokes an effect operation.

```
// 1. Define the effect
Panic: effect {
    panic: (msg: str) -> !
}

// 2. Use the effect in a function
division: (a: i32, b: i32) -> i32 with {Panic} {
    if b == 0 {
        perform Panic.panic("division by zero")
    }
    a / b
}
```

### **`handler` and `with`**

A **`handler`** provides the concrete implementation for an effect's operations. The `with` keyword applies a handler to a function call. Handlers can be defined globally or in-place for a specific call.

```
// 1. Define a handler
OsExitPanic: handler Panic {
    panic: (msg: str) -> ! {
        fmt.println(msg)
        os.exit(1)
    }
}

// 2. Apply the handler at the call site
// This call will exit with code 1 if division by zero occurs.
out := division(10, 0) with {OsExitPanic}

// An in-place handler can also be provided.
out2 := division(10, 2) with {
    panic: (msg: str) -> ! {
        fmt.println("panic handled gracefully")
        os.exit(0)
    }
}
```
This system is composable, allowing complex behaviors like asynchronous operations to be modeled cleanly.

---
## **9. Runtime & Metaprogramming**

* **Entry Point**: A program must contain a `fn main() -> i32`.
* **Compile-Time Execution**: A `meta` block allows code to be executed at compile time. This is useful for conditional compilation or generating code based on the target environment.

```
// 'a' will be 4 when compiling for WASM, and 8 otherwise.
a := meta {
    if runtime.is_wasm() {
        4
    } else {
        8
    }
}
```

---
## **10. Documentation**

Documentation comments use `///` and apply to the following item. Tooling can parse these comments to generate documentation.

```
/// Represents a user of the system.
UserType: enum { ... }
