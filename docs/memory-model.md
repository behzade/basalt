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
fn main() -> i32 {
    memory catalog: chunk(67108864)
    let scratch = "short lived"
    let records in catalog = load_catalog()
    0
}
```

In this example, `scratch` belongs to the current function's temporary frame. `records` belongs to
the lexical `catalog` chunk. The region is destroyed when its declaring block exits.

## Contexts And Regions

A memory context is a lifetime and accounting unit. Contexts form a tree.

- Global/root context lives for the program.
- Function contexts are short-lived stack-like children.
- Lexically declared chunk contexts have explicit size/object budgets and block-bounded lifetimes.
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
- Current block-statement syntax is `memory name: chunk(bytes[, objects])`.
- `reset name` rewinds the chunk and advances its allocation generation.
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

The interpreter represents these as distinct root contexts. The runtime root cannot own user
`Value`s. Its child global-user context is budgeted to 64 KiB and 1024 objects and is used only as
an explicit program-lifetime destination, currently for an outermost compound return. Ordinary
allocation with no active frame or named region is an interpreter error rather than an implicit
global allocation. User-facing global declarations are not implemented yet.

This avoids recursive questions such as where memory-region descriptors are allocated.

## Placement

Every allocation has a destination.

```text
let x = expr            -> current temp context
let x in region = expr  -> mutable Region symbol in lexical scope
return expr             -> caller-provided return destination
```

The caller controls where returned owned values live. A function may use temporary memory for intermediate values, but its returned owned value must be constructed into the caller destination or an explicit named destination.

## Pipelines

Performance-sensitive pipelines should create an explicit named context for the working set.

```bst
fn parse_request(mut region: Region, body: str) -> i32 {
    let tokens in region = lex(body)
    let ast in region = parse(tokens)
    let checked in region = typecheck(ast)
    checked.score()
}

fn handle_request(body: str) -> i32 {
    memory request: chunk(524288)
    parse_request(request, body)
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

The interpreter now treats bindings as destination-aware views. An immutable compound parameter
observes its source value without changing its owner. A mutable parameter aliases one caller
binding, retains that binding's allocation destination, and copies assigned compound values into
that destination before the callee can publish them. Passing the same binding to two mutable
parameters in one call is rejected by the typechecker and checked again by the interpreter.

`let value in Region = existing` always copies/reconstructs `existing` into `Region`, including
when the initializer is only a path. Struct field assignment similarly places the new field value
in the struct owner's context. These rules prevent bindings and fields from retaining callee or
sibling-owned temporaries.

Regions themselves are opaque lexical capability values. `memory name: chunk(...)` introduces an
inherently mutable `Region` symbol in the current block. A function must receive
`mut region: Region` to allocate into or reset the caller's region; a plain `Region` parameter has
no allocation authority. Region capabilities cannot be returned or stored in structs. Runtime
mutable-call checks compare underlying region identity, so two different aliases cannot lend the
same region mutably in one call.

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

The interpreter currently implements Basalt-owned region allocation policy plus host-side value
provenance, not physical placement of every value into its reserved address.

- Heap-shaped values are charged to the active function frame.
- Every lexical chunk owns a private mutable `std::runtime::allocator::Arena`. Region creation,
  byte/object exhaustion, allocation, reset, and release execute the Basalt functions directly;
  the Rust interpreter retains only opaque context identity/generation and the private allocator
  binding. Runtime allocator HIR is compiled separately and cannot become visible through user
  imports or unqualified name lookup.
- The internal `std::runtime::raw` submodule contains the small hosted `libc_*` process-memory
  boundary and opaque address operations.
- Process addresses have the nominal `MemoryAddress` type. Their machine-number representation is
  host-private; user functions cannot return them, store them in user structs, or bind them outside
  the lexical `unsafe` scope where they are used.
- Interpreter addresses carry allocation identity, generation, and byte offset. Every offset,
  memory operation, and free validates liveness and bounds; use-after-free, double-free, interior
  free, and out-of-bounds ranges are runtime errors before libc is invoked.
- `std::runtime::buffer::Buffer` is the first address-backed runtime value. Its allocation and byte
  access use checked addresses, while Basalt code owns capacity and initialized-length policy. The
  module is internal; ordinary programs cannot import the raw Buffer lifecycle API.
- Interpreter allocation contexts now have stable identities and generations. Heap-shaped values
  record their allocation context, and every read checks that the recorded generation is live.
- Contexts record explicit parents: the user-global context belongs to the host-private runtime
  root, nested call frames belong to their caller frame, and a lexical region belongs to the
  context active where its `memory` statement executes.
- Interpreter binding reads use one checked observation path. Bindings retain their allocation
  destination, so whole-value and field mutation reconstruct compound values in the owner context.
- Function literals capture only referenced free variables. When a closure crosses an allocation
  context, its captured bindings are detached and reconstructed into the closure destination;
  mutable captured state remains shared by subsequent calls to that closure.
- Function results are copied/reconstructed into the caller's active destination before the callee
  frame is invalidated. `let x in Region = call()` therefore makes `Region` the return destination,
  while an ordinary call returns into the caller's temporary frame.
- A callee that writes a compound value through a mutable caller alias reconstructs that value in
  the caller binding's allocation destination before the callee frame exits.
- `reset region` requires a mutable Region binding, calls Basalt `arena_reset`, and advances its
  context generation. Existing values retain the prior generation and are rejected on their next
  read.
- Exiting the declaring lexical block copies an owned block result into the outer destination,
  calls Basalt `arena_release`, invalidates remaining region values, and destroys the private
  allocator binding.
- Scalars are free.
- Strings, arrays, maps, structs, enum variants, closures, and handlers count against the frame.
- Exceeding the frame budget is a runtime error.

The arena returns real process addresses, but ordinary strings, structs, arrays, and other compound
interpreter `Value`s are not stored at those addresses yet. They now obey context provenance and
return placement in the interpreter, which separates lifetime semantics from physical layout.
Moving their bytes to internal address-backed representations remains the next storage integration
step.

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
- Dynamic region nesting beyond lexical blocks still needs precise rules.
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

- How should return destination be represented in HIR?
- How much region information belongs in user-facing types versus internal HIR metadata?
- Which effects are guaranteed non-suspending, and how is that represented?
