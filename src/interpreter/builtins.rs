use std::io::Write as _;

use crate::hir::{Expr, ExprKind};

use super::env::{Env, Result, RuntimeError};
use super::value::Value;

pub fn try_builtin_call(name: &str, args: &Vec<Expr>, env: &mut Env) -> Result<Option<Value>> {
    match name {
        // std::runtime::exit(code: i32) -> !
        "exit" => {
            if args.len() != 1 { return Err(RuntimeError("exit expects 1 argument".into())); }
            let code = eval_expr_shallow(&args[0], env)?;
            let code_i32 = match code {
                Value::I32(i) => i,
                Value::I64(i) => i as i32,
                _ => return Err(RuntimeError("exit expects i32".into())),
            };
            std::process::exit(code_i32);
        }
        // std::runtime::write(stream: str, input: str) -> ()
        // stream one of: "stdout", "stderr", or a file path
        "write" => {
            if args.len() != 2 { return Err(RuntimeError("write expects 2 arguments".into())); }
            let stream_val = eval_expr_shallow(&args[0], env)?;
            let data_val = eval_expr_shallow(&args[1], env)?;
            let stream = match stream_val { Value::Str(s) => s, _ => return Err(RuntimeError("write stream must be str".into())) };
            let data_raw = match data_val { Value::Str(s) => s, _ => return Err(RuntimeError("write data must be str".into())) };
            let data = unescape_runtime_string(&data_raw);
            match stream.as_str() {
                "stdout" => {
                    let mut out = std::io::stdout();
                    out.write_all(data.as_bytes()).map_err(|e| RuntimeError(e.to_string()))?;
                    out.flush().ok();
                }
                "stderr" => {
                    let mut out = std::io::stderr();
                    out.write_all(data.as_bytes()).map_err(|e| RuntimeError(e.to_string()))?;
                    out.flush().ok();
                }
                path => {
                    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)
                        .map_err(|e| RuntimeError(e.to_string()))?;
                    f.write_all(data.as_bytes()).map_err(|e| RuntimeError(e.to_string()))?;
                }
            }
            Ok(Some(Value::Unit))
        }
        _ => Ok(None),
    }
}

fn eval_expr_shallow(expr: &Expr, env: &mut Env) -> Result<Value> {
    match &expr.kind {
        ExprKind::Literal(pty, text) => super::eval::eval_literal_direct(pty, text),
        // Fall back to full evaluation path for non-literals
        _ => super::eval::eval_expr_full(expr, env),
    }
}

/// Convert common escape sequences in runtime strings (e.g., "\n") into their
/// actual characters. This is applied in the interpreter's magic write function
/// so source strings like "hi\n" produce a newline at runtime.
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
                    // Unknown escape: keep the backslash and the char
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
        // Trailing backslash: keep it as-is
        result.push('\\');
    }
    result
}


