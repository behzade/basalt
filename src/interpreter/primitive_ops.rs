use crate::hir::Expr;

use super::env::{Env, Result, RuntimeError};
use super::value::Value;

pub(crate) fn try_primitive_call<F>(
    name: &str,
    args: &[Expr],
    env: &mut Env,
    mut eval_expr: F,
) -> Result<Option<Value>>
where
    F: FnMut(&Expr, &mut Env) -> Result<Value>,
{
    match name {
        "len" => {
            if args.len() != 1 {
                return Err(RuntimeError("len expects 1 argument".into()));
            }
            let value = eval_expr(&args[0], env)?;
            match value {
                Value::Str(s) => Ok(Some(Value::I32(s.chars().count() as i32))),
                Value::Array(items) => Ok(Some(Value::I32(items.len() as i32))),
                Value::Map(entries) => Ok(Some(Value::I32(entries.len() as i32))),
                other => Err(RuntimeError(format!("len unsupported for {}", other))),
            }
        }
        _ => Ok(None),
    }
}
