pub mod checker;
pub mod errors;
pub mod lowering;
pub mod registry;
pub mod scope;
pub mod symbols;
pub mod unify;
pub mod utils;

pub use checker::Typechecker;
pub use errors::{ItemContext, TypeError};
