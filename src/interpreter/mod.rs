mod env;
mod eval;
mod primitive_ops;
mod runtime;
mod stack;
mod value;

pub use eval::run_program;
pub use value::{Value, value_to_exit_code};
