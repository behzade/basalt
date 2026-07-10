use super::env::{Result, RuntimeError};
use super::value::Value;

pub(crate) fn contains(name: &str) -> bool {
    name == "len"
}

pub(crate) fn try_primitive_call(name: &str, args: &[Value]) -> Result<Option<Value>> {
    match name {
        "len" => {
            if args.len() != 1 {
                return Err(RuntimeError("len expects 1 argument".into()));
            }
            let value = args[0].clone();
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
