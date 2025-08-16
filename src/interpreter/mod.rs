mod value;
mod env;
mod eval;

pub use value::{Value, FunctionValue, value_to_exit_code};
pub use env::{Env, RuntimeError, Result};
pub use eval::{Interpreter, run_program};


