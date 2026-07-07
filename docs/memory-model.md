# Basalt Memory Model Sketch

This document records the current memory-system direction. It is not final language law. The purpose is to make the hypothesis precise enough that interpreter experiments can falsify it.

## Goals

- Make allocation visible enough that large memory use has a name.
- Keep ordinary small code ergonomic.
- Avoid a tracing GC or reference counting as the first runtime substrate.
- Avoid Rust-style user-facing lifetimes as the first safety mechanism.
- Support future effects, async, and named working sets without hiding memory in a default heap.

## Core Idea

Basalt allocation is charged to a memory context.

The default context for ordinary function-local allocations is stack-like temporary memory. Large or long-lived data should be allocated into explicit named chunk contexts.

```bst
memory Catalog: chunk(67108864)

fn main() -> i32 {
    let scratch = "short lived"
    let catalog in Catalog = load_catalog()
    0
}
```

In this example, `scratch` belongs to the current function's temporary frame. `catalog` belongs to the named `Catalog` chunk.

## Contexts And Regions

A memory context is a lifetime and accounting unit. Contexts form a tree.

- Global/root context lives for the program.
- Function contexts are short-lived stack-like children.
- Named chunk contexts have explicit size/object budgets and explicit lifetimes.
- Child contexts may read from ancestor contexts.
- Ancestor contexts may not retain references to child contexts.
- Sibling contexts may not reference each other directly.

The central safety rule is:

```text
A reference may point to data in the same context or an ancestor context.
A reference may not point to data in a descendant or sibling context.
```

## Allocator Kinds

The first allocator kinds are composable but simple.

### Temp/Stack Allocator

- Bump allocation.
- Rewind on function or scope exit.
- No individual free.
- Small default byte and object limits.
- Used for short-lived local values.

### Chunk Allocator

- Explicit size.
- Current syntax is `memory Name: chunk(bytes[, objects])`.
- Bump allocation inside a named chunk.
- Reset or destroy whole chunk.
- No individual free initially.
- Used for large or long-lived working sets.

Future allocator kinds may include slabs, pools, tracing regions, or no-allocation regions. They should compose as memory contexts rather than become unrelated runtime mechanisms.

## Global Memory

Global memory should be small by default. It is not the place for accidental large objects.

There are two separate concepts:

- Runtime control memory: internal metadata for context descriptors, allocator state, interpreter/bookkeeping. This is not charged as user memory.
- Global user memory: program-visible global allocations, with a low budget.

This avoids recursive questions such as where memory-region descriptors are allocated.

## Placement

Every allocation has a destination.

```text
let x = expr            -> current temp context
let x in Region = expr  -> named region
return expr             -> caller-provided return destination
```

The caller controls where returned owned values live. A function may use temporary memory for intermediate values, but its returned owned value must be constructed into the caller destination or an explicit named destination.

## Pipelines

Performance-sensitive pipelines should create an explicit named context for the working set.

```bst
memory Request: chunk(524288)

fn handle_request(body: str) -> i32 {
    with context Request {
        let tokens = lex(body)
        let ast = parse(tokens)
        let checked = typecheck(ast)
        checked.score()
    }
}
```

The goal is that diagnostics can report memory by program concept:

```text
Request: 418kb / 512kb, 920 objects
Catalog: 62mb / 64mb, 100231 objects
```

## References And Views

References are not the primary ownership mechanism. Owned values crossing contexts should be copied or constructed into the destination region.

Allowed references:

- Readonly references from a shorter-lived context to an ancestor context.
- Write references from a shorter-lived context to an ancestor context only with exclusivity.

Forbidden references:

- Ancestor to descendant.
- Sibling to sibling.
- References stored in longer-lived objects if they point to shorter-lived data.

Initial references should be stack-only views:

- They may be parameters and local temporaries.
- They may not be stored in region-owned objects.
- They may not be returned.
- They may not be captured by escaping closures.
- They may not cross suspension points.

This keeps the first model away from full Rust-style lifetime syntax while preserving useful no-copy reads and exclusive mutation.

## Effects, Async, And Stack Unrolling

The temp stack model is straightforward for normal synchronous calls. It becomes constrained at dynamic control boundaries:

- function return
- panic/unwind
- effect suspension
- async spawn
- escaping closure

Anything that survives stack unrolling must live in a context that outlives the unrolled frames.

Initial constraints:

- Non-suspending effects may run on stack frames.
- Suspending effects must move/copy live state into a continuation or task context.
- Async tasks must have an explicit task context.
- Escaping closures must allocate captures into an explicit destination context.

## Current Interpreter Experiment

The interpreter currently implements accounting plus runtime-backed chunk reservation, not physical placement of every value into that chunk.

- Heap-shaped values are charged to the active function frame.
- Named chunk declarations reserve their backing byte budget through `std::runtime::alloc`.
- Scalars are free.
- Strings, arrays, maps, structs, enum variants, closures, and handlers count against the frame.
- Exceeding the frame budget is a runtime error.

Example runnable boundary:

```bst
fn main() -> i32 {
    let message = "stack string"
    message.len()
}
```

Example failing boundary:

```bst
fn main() -> i32 {
    let large = "..."
    large.len()
}
```

The failure demonstrates that unnamed local allocation is intentionally small. The language needs named chunks before large local working sets become practical.

## First Implementation Constraints

To avoid use-after-free and other memory bugs in the first real version:

- Do not expose raw pointers.
- Do not expose general references.
- Compound values own their contents.
- Cross-region placement deep-copies or constructs into the destination region.
- Function returns allocate into caller-provided destination.
- Region reset invalidates all objects in that region.
- Interpreter/debug mode tags objects with region and generation for stale-handle checks.

Only after these constraints are working should Basalt add stored references, slices with region metadata, async captures, or allocator kinds with individual free.

## Review Findings

An independent review pass judged this model plausible rather than nonsensical. The closest existing family is region-based memory with stack-like temporary regions, named arenas, and Rust-like aliasing restrictions for references.

Strong arguments for the model:

- The reference direction rule matches the core invariant behind stack allocation, regions, and borrow checking.
- Named chunks give memory use a program concept instead of hiding everything in a heap.
- Temp-by-default plus explicit long-lived placement matches phase-oriented workloads such as compilers, games, request handling, query engines, and interpreters.
- Readonly ancestor borrows and exclusive mutable ancestor borrows are a coherent smaller subset of Rust's aliasing model.

Main risks:

- Escape behavior can become surprising unless destinations are explicit.
- Mutable ancestor references can grow into a borrow checker if Basalt allows arbitrary stored references.
- Chunk lifetimes need precise creation, reset, destruction, and nesting rules.
- Globals can break the model if they can point into ordinary chunks or temporary frames.
- Cross-region copying needs clear copy, move, ownership, and destructor semantics.

Near-term recommendations from the review:

- Treat this as a region model, not only an allocator model.
- Start with coarse rules even if they reject some useful programs.
- Keep globals boring: static/scalar data or explicit immortal/global-region allocation only.
- Make chunk reset checked in the interpreter.
- Forbid closure capture of temp references in the first version.
- Allow owned/copied return values before reference returns.
- Treat mutable ancestor access as exclusive at function-call boundaries before trying field-sensitive checking.

## Open Questions

- Should `with context Region` be lexical only, dynamic only, or both?
- Should region names be compile-time declarations only, or can code create dynamic region handles?
- What is the first syntax for region declarations and byte sizes?
- How should return destination be represented in HIR?
- How much region information belongs in user-facing types versus internal HIR metadata?
- Which effects are guaranteed non-suspending, and how is that represented?
