use std::io::Write as _;

use crate::hir::Expr;

use super::env::{Env, Result, RuntimeError};
use super::value::Value;

pub(crate) fn try_builtin_call<F>(
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
        "exit" => {
            if args.len() != 1 {
                return Err(RuntimeError("exit expects 1 argument".into()));
            }
            let code = eval_expr(&args[0], env)?;
            let code_i32 = match code {
                Value::I32(i) => i,
                Value::I64(i) => i as i32,
                _ => return Err(RuntimeError("exit expects i32".into())),
            };
            std::process::exit(code_i32);
        }
        "write" => {
            if args.len() != 2 {
                return Err(RuntimeError("write expects 2 arguments".into()));
            }
            let stream_val = eval_expr(&args[0], env)?;
            let data_val = eval_expr(&args[1], env)?;
            let stream = match stream_val {
                Value::Str(s) => s,
                _ => return Err(RuntimeError("write stream must be str".into())),
            };
            let data_raw = match data_val {
                Value::Str(s) => s,
                _ => return Err(RuntimeError("write data must be str".into())),
            };
            let data = unescape_runtime_string(&data_raw);
            match stream.as_str() {
                "stdout" => {
                    let mut out = std::io::stdout();
                    out.write_all(data.as_bytes())
                        .map_err(|e| RuntimeError(e.to_string()))?;
                    out.flush().ok();
                }
                "stderr" => {
                    let mut out = std::io::stderr();
                    out.write_all(data.as_bytes())
                        .map_err(|e| RuntimeError(e.to_string()))?;
                    out.flush().ok();
                }
                path => {
                    let mut f = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .map_err(|e| RuntimeError(e.to_string()))?;
                    f.write_all(data.as_bytes())
                        .map_err(|e| RuntimeError(e.to_string()))?;
                }
            }
            Ok(Some(Value::Unit))
        }
        _ => Ok(None),
    }
}

fn unescape_runtime_string(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut escaping = false;
    for ch in input.chars() {
        if escaping {
            match ch {
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                '0' => result.push('\0'),
                other => {
                    result.push('\\');
                    result.push(other);
                }
            }
            escaping = false;
        } else if ch == '\\' {
            escaping = true;
        } else {
            result.push(ch);
        }
    }
    if escaping {
        result.push('\\');
    }
    result
}
