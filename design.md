# **Basalt Language Specification (MVP)**

## **1\. Philosophy**

**Basalt** is a statically-typed, expression-oriented language designed for safety, developer productivity, and portability by targeting WebAssembly (WASM). It achieves safety through a shared ownership memory model and a typed effects system, and productivity through clear syntax and built-in documentation tooling.

## **2\. Syntax and Variables**

Basalt is expression-oriented. A semicolon ; turns an expression into a statement, discarding its value. The last expression in a block is its return value.

### **Declarations**

There are three ways to declare a name:

* let: Binds a variable to an immutable runtime value.  
* let mut: Binds a variable to a mutable runtime value.  
* const: Defines a compile-time constant. Its value must be known at compile time.

const PI: f32 \= 3.14159;

fn main() \-\> i32 {  
    let x: i32 \= 10;  
    let mut y: i32 \= 20;  
    y \= y \+ x; // OK: y is mutable  
      
    // x \= 5; // COMPILE ERROR: x is immutable  
      
    let z: i32 \= {  
        let inner \= y \+ 5;  
        inner \- x // This is the return value of the block  
    }; // z is 15  
      
    return 0;  
}

### **Definite Assignment**

Basalt enforces **definite assignment analysis**. It is a **compile-time error** to read from a variable before the compiler can prove it has been assigned a value on every possible code path. There are no default null or zeroed values.

fn example() {  
    let x: i32;  
    if some\_condition() {  
        x \= 10;  
    }  
    // COMPILE ERROR: 'x' may not have been initialized  
    // if some\_condition() was false.  
    let y \= x;   
}

## **3\. Types and Data Structures**

All types must be explicitly annotated. The compiler performs no type inference.

### **Primitives**

* i8,i16,i32,i64: Signed integers.
* u8,u16,u32,u64: Unsigned integers.
* f32,f64: Floating-point numbers.
* bool: true or false.  
* (): The "unit" type, representing no value.  
* \!: The "never" type. It indicates that a function will never return a value, typically by aborting or entering an infinite loop.

### **Composite Types**

**Structs** define a product type. Fields are private by default.

```
pub struct Point {  
    pub x: f32,  
    y: f32, // Private field  
}
```

**Enums** define a sum type (tagged union). Option\<T\> and Result\<T, E\> are available in the global namespace.

```
enum Option\<T\> {  
    Some(T),  
    None,  
}
```

### **Type Aliases**

The type keyword creates a new name for an existing type to improve readability.

```
type UserID \= i32;  
type Name \= string;
```

### **Collections and Views**

Basalt distinguishes between owning data and viewing it.

* Vec\<T\>: An owned, heap-allocated, resizable buffer. Managed via shared ownership.  
* Slice\<T\>: A non-owning, immutable view into a contiguous block of memory. A Slice is a lightweight struct containing a pointer and length, and is passed by value.  
* string: Defined in the standard library via a type alias: pub type string \= Slice\<u8\>;. String literals are Slice\<u8\> pointing to static memory.

## **4\. Memory Model: Shared Ownership & CoW**

Basalt uses a **shared ownership** model with atomic reference counting for all complex data types (Vec, Map, structs).

When a value bound to a mut variable is mutated, a **Copy-on-Write (CoW)** policy is enforced.

* If the reference count of the data is 1, the data is mutated in place.  
* If the reference count is greater than 1, the data is copied before mutation.

## **5\. Behavior and Abstraction: Traits & Impls**

trait and impl are the mechanisms for defining behavior on types.

### **trait: Defining Shared Behavior**

A **trait** defines an interface—a set of function signatures that a type can implement.

pub trait Serializable {  
    fn serialize(self) \-\> string;  
}

### **impl: Providing Implementations**

An **impl** block is used to implement traits for a type or to define methods directly on a type.

// Implement the Serializable trait for the Point struct  
impl Serializable for Point {  
    fn serialize(self) \-\> string {  
        // ... implementation for serializing a Point  
    }  
}

// Implement methods directly on Point  
impl Point {  
    // An "associated function" (like a static method)  
    pub fn new(x: f32, y: f32) \-\> Point {  
        Point { x: x, y: y }  
    }

    // A "method" that takes self  
    pub fn distance\_from\_origin(self) \-\> f32 {  
        // ... implementation  
    }  
}

*Trait objects (let a: Serializable;) for dynamic dispatch are a planned post-MVP feature.*

## **6\. Functions & Control Flow**

Functions are declared with fn. Higher-order functions are supported by assigning anonymous, non-capturing function declarations to variables.

fn add(a: i32, b: i32) \-\> i32 {  
    return a \+ b;  
}

Control flow is handled with if/else, while, and match. The \! type is useful for ensuring exhaustive match arms are type-correct.

fn panic(message: string) \-\> \! {  
    // A built-in function that prints a message and aborts the program.  
}

let val: Option\<i32\> \= Option::Some(10);  
let result: i32 \= match val {  
    Option::Some(x) \=\> x,  
    // This branch never completes, so it doesn't need to produce an i32.  
    Option::None \=\> panic("Value was None\!"),  
};

## **7\. Modularity and Visibility**

* **Modules**: A directory of .bst files constitutes a single module and namespace. import statements are path-based.  
* **Visibility**: All items (fn, struct, enum, trait, const) and struct fields are **private by default**. The pub keyword makes an item visible outside its module.

## **8\. Error Handling & Effects**

* **Recoverable Errors**: Handled using the Result\<T, E\> enum.  
* **Systemic Effects**: Handled with a typed algebraic effects system. A function that can perform an effect must declare it in its signature.

effect Fs {  
    fn read(path: string) \-\> string;  
}

fn read\_config() \-\> string / {Fs} {  
    let content: string \= perform Fs::read("./config.json");  
    return content;  
}

## **9\. Runtime & FFI**

* **Entry Point**: A program must contain a fn main() \-\> i32 or fn main(args: Vec\<string\>) \-\> i32.  
* **Panics**: Unrecoverable runtime errors cause the program to **abort** immediately. Functions that perform this action return \!.  
* **FFI**: Foreign functions are declared in an extern block and must be called from within an unsafe block.

## **10\. Documentation**

Documentation comments use /// and apply to the following item. Tooling can parse these comments to generate documentation.

/// Represents a point in a 2D coordinate space.  
pub struct Point {  
    pub x: f32,  
    y: f32,  
}  

