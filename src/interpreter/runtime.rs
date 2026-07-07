use crate::hir;

use super::env::{Result, RuntimeError};
use super::value::Value;

pub(crate) fn call_runtime_intrinsic(
    function: &hir::HirFunction,
    args: Vec<Value>,
) -> Result<Value> {
    let name = function.signature.name.as_str();
    match name {
        "exit" => {
            let [code] = args.as_slice() else {
                return Err(RuntimeError("runtime::exit expects 1 argument".into()));
            };
            let code = match code {
                Value::I32(i) => *i,
                Value::I64(i) => *i as i32,
                _ => return Err(RuntimeError("runtime::exit expects i32".into())),
            };
            unsafe {
                libc::exit(code);
            }
        }
        "write" => {
            let [stream, data] = args.as_slice() else {
                return Err(RuntimeError("runtime::write expects 2 arguments".into()));
            };
            let stream = match stream {
                Value::Str(s) => s.as_str(),
                _ => return Err(RuntimeError("runtime::write stream must be str".into())),
            };
            let data = match data {
                Value::Str(s) => unescape_runtime_string(s),
                _ => return Err(RuntimeError("runtime::write data must be str".into())),
            };
            let fd = match stream {
                "stdout" => libc::STDOUT_FILENO,
                "stderr" => libc::STDERR_FILENO,
                other => return Err(RuntimeError(format!("Unknown runtime stream: {}", other))),
            };
            write_all(fd, data.as_bytes())?;
            Ok(Value::Unit)
        }
        "alloc" => {
            let [bytes] = args.as_slice() else {
                return Err(RuntimeError("runtime::alloc expects 1 argument".into()));
            };
            let bytes = value_to_usize(bytes, "runtime::alloc bytes")?;
            let ptr = alloc_bytes(bytes)?;
            Ok(Value::U64(ptr))
        }
        "free" => {
            let [ptr, bytes] = args.as_slice() else {
                return Err(RuntimeError("runtime::free expects 2 arguments".into()));
            };
            let ptr = value_to_u64(ptr, "runtime::free ptr")?;
            let bytes = value_to_usize(bytes, "runtime::free bytes")?;
            free_bytes(ptr, bytes);
            Ok(Value::Unit)
        }
        other => Err(RuntimeError(format!(
            "Unknown runtime intrinsic: std::runtime::{}",
            other
        ))),
    }
}

pub(crate) fn alloc_bytes(bytes: usize) -> Result<u64> {
    if bytes == 0 {
        return Err(RuntimeError(
            "runtime::alloc requires a non-zero byte count".to_string(),
        ));
    }
    let ptr = unsafe { libc::malloc(bytes as libc::size_t) };
    if ptr.is_null() {
        return Err(RuntimeError(format!(
            "runtime::alloc failed to allocate {} bytes",
            bytes
        )));
    }
    Ok(ptr as u64)
}

pub(crate) fn free_bytes(ptr: u64, _bytes: usize) {
    if ptr == 0 {
        return;
    }
    unsafe {
        libc::free(ptr as *mut libc::c_void);
    }
}

fn value_to_usize(value: &Value, context: &str) -> Result<usize> {
    let n = value_to_u64(value, context)?;
    usize::try_from(n).map_err(|_| RuntimeError(format!("{} does not fit usize", context)))
}

fn value_to_u64(value: &Value, context: &str) -> Result<u64> {
    match value {
        Value::U64(n) => Ok(*n),
        Value::U32(n) => Ok((*n).into()),
        Value::U16(n) => Ok((*n).into()),
        Value::U8(n) => Ok((*n).into()),
        Value::I32(n) if *n >= 0 => Ok(*n as u64),
        Value::I64(n) if *n >= 0 => Ok(*n as u64),
        other => Err(RuntimeError(format!(
            "{} must be a non-negative integer, got {}",
            context,
            other.allocation_kind()
        ))),
    }
}

fn write_all(fd: libc::c_int, mut bytes: &[u8]) -> Result<()> {
    while !bytes.is_empty() {
        let written = unsafe {
            libc::write(
                fd,
                bytes.as_ptr().cast::<libc::c_void>(),
                bytes.len() as libc::size_t,
            )
        };
        if written < 0 {
            return Err(RuntimeError(std::io::Error::last_os_error().to_string()));
        }
        if written == 0 {
            return Err(RuntimeError("runtime::write wrote zero bytes".to_string()));
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
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
