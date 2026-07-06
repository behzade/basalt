mod env;
mod eval;
mod value;

pub use env::{Env, Result, RuntimeError};
pub use eval::{Interpreter, run_program};
pub use value::{FunctionValue, Value, value_to_exit_code};
