// std/collections/vec module (placeholder). Generics not supported in this pass.

type Vec = { length: i32 }

fn new() -> Vec { { length: 0 } }

fn push(vec: Vec, _item: i32) -> Vec { vec }

fn len(vec: Vec) -> i32 { vec.length }