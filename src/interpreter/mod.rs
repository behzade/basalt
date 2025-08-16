use std::collections::HashMap;
use std::io::Write as _;

use crate::hir::{self, BinaryOp, Expr, ExprKind, HirBlock, HirFunction, Stmt, UnaryOp};

/// A simple runtime value for the tree-walking interpreter.
#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Byte(u8),
    I32(i32),
    I64(i64),
    F64(f64),
    Str(String),
    Array(Vec<Value>),
    Function(FunctionValue),
}

#[derive(Debug)]
pub struct RuntimeError(pub String);

type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Default, Debug, Clone)]
struct Env {
    scopes: Vec<HashMap<String, Value>>, // lexical scopes, last is current
}

impl Env {
    fn new() -> Self {
        Env { scopes: vec![HashMap::new()] }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String, value: Value) {
        if let Some(current) = self.scopes.last_mut() {
            current.insert(name, value);
        }
    }

    fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(RuntimeError(format!("Undefined variable: {}", name)))
    }

    fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}

/// Captures a returned value from inside a block/function to unwind control flow.
enum ControlFlow {
    Next,
    Return(Value),
}

/// A very small tree-walking interpreter over the HIR.
pub struct Interpreter {
    functions: HashMap<String, HirFunction>,
}

impl Interpreter {
    pub fn new(items: &[hir::Item]) -> Self {
        let mut functions = HashMap::new();
        for item in items {
            if let hir::Item::Fn(func) = item {
                functions.insert(func.signature.name.clone(), func.clone());
            }
        }
        Interpreter { functions }
    }

    /// Runs the program by looking for a top-level function named `main`.
    /// Returns `Unit` if no `main` is present.
    pub fn run(&self) -> Result<Value> {
        if let Some(main_fn) = self.functions.get("main") {
            self.call_function(main_fn, vec![])
        } else {
            Ok(Value::Unit)
        }
    }

    fn call_function(&self, function: &HirFunction, args: Vec<Value>) -> Result<Value> {
        if !function.signature.params.is_empty() && function.signature.params.len() != args.len() {
            return Err(RuntimeError(format!(
                "Function '{}' expected {} arguments, got {}",
                function.signature.name,
                function.signature.params.len(),
                args.len()
            )));
        }

        let mut env = Env::new();
        // bind params
        for (idx, param) in function.signature.params.iter().enumerate() {
            if let Some(arg_val) = args.get(idx).cloned() {
                env.define(param.name.clone(), arg_val);
            }
        }

        self.eval_block(&function.body, &mut env)
            .map(|v| v.unwrap_or(Value::Unit))
    }

    fn call_function_value(&self, function: &FunctionValue, args: Vec<Value>) -> Result<Value> {
        if function.params.len() != args.len() {
            return Err(RuntimeError(format!(
                "Function literal expected {} arguments, got {}",
                function.params.len(),
                args.len()
            )));
        }
        // Recreate environment with captured scopes and a fresh top scope for parameters
        let mut env = Env { scopes: function.captured.clone() };
        env.push_scope();
        for (idx, param) in function.params.iter().enumerate() {
            env.define(param.name.clone(), args[idx].clone());
        }
        self.eval_block(&function.body, &mut env)
            .map(|v| v.unwrap_or(Value::Unit))
    }

    /// Evaluate a block, returning the last expression value or an explicit return value.
    fn eval_block(&self, block: &HirBlock, env: &mut Env) -> Result<Option<Value>> {
        env.push_scope();
        for stmt in &block.stmts {
            match self.eval_stmt(stmt, env)? {
                ControlFlow::Next => {}
                ControlFlow::Return(v) => {
                    env.pop_scope();
                    return Ok(Some(v));
                }
            }
        }
        let result = if let Some(last) = &block.last_expr {
            Some(self.eval_expr(last, env)?)
        } else {
            None
        };
        env.pop_scope();
        Ok(result)
    }

    fn eval_stmt(&self, stmt: &Stmt, env: &mut Env) -> Result<ControlFlow> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let v = if let Some(expr) = value { self.eval_expr(expr, env)? } else { Value::Unit };
                env.define(name.clone(), v);
                Ok(ControlFlow::Next)
            }
            Stmt::Return { value, .. } => {
                let ret = if let Some(expr) = value { self.eval_expr(expr, env)? } else { Value::Unit };
                Ok(ControlFlow::Return(ret))
            }
            Stmt::Assign { lhs, rhs, .. } => {
                // Only support simple identifier assignment for now
                if let ExprKind::Path(path) = &lhs.kind {
                    if let Some(var_name) = path.last() {
                        let new_val = self.eval_expr(rhs, env)?;
                        env.assign(var_name, new_val)?;
                        return Ok(ControlFlow::Next);
                    }
                }
                Err(RuntimeError("Unsupported assignment target".to_string()))
            }
            Stmt::Expr { expr, .. } => {
                let _ = self.eval_expr(expr, env)?;
                Ok(ControlFlow::Next)
            }
            Stmt::Error { .. } => Err(RuntimeError("Encountered error statement in HIR".to_string())),
        }
    }

    fn eval_expr(&self, expr: &Expr, env: &mut Env) -> Result<Value> {
        match &expr.kind {
            ExprKind::Literal(pty, text) => self.eval_literal(pty, text),
            ExprKind::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval_expr(item, env)?);
                }
                Ok(Value::Array(values))
            }
            ExprKind::Map(_) => Err(RuntimeError("Map not yet supported".to_string())),
            ExprKind::Path(path) => {
                // Variable lookup for simple paths
                if let Some(name) = path.last() {
                    if let Some(val) = env.get(name) {
                        return Ok(val);
                    }
                    // A bare path could also be a function, in which case return Unit for now
                    if self.functions.contains_key(name) {
                        return Ok(Value::Unit);
                    }
                }
                Err(RuntimeError(format!("Unknown path: {:?}", path)))
            }
            ExprKind::FieldAccess { .. } => Err(RuntimeError("Field access not yet supported".to_string())),
            ExprKind::Unary { op, rhs } => {
                let v = self.eval_expr(rhs, env)?;
                self.apply_unary(*op, v)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lv = self.eval_expr(lhs, env)?;
                let rv = self.eval_expr(rhs, env)?;
                self.apply_binary(*op, lv, rv)
            }
            ExprKind::Call { fun, args } => {
                // Evaluate callee; support calling top-level and function values
                if let ExprKind::Path(p) = &fun.kind {
                    if let Some(name) = p.last() {
                        // Builtins: minimal host I/O
                        if let Some(builtin) = self.try_builtin_call(name, args, env)? {
                            return Ok(builtin);
                        }
                        if let Some(f) = self.functions.get(name) {
                            let mut evaled = Vec::with_capacity(args.len());
                            for a in args {
                                evaled.push(self.eval_expr(a, env)?);
                            }
                            return self.call_function(f, evaled);
                        }
                        // Variable binding holding a function value
                        if let Some(Value::Function(func_val)) = env.get(name) {
                            let mut evaled = Vec::with_capacity(args.len());
                            for a in args {
                                evaled.push(self.eval_expr(a, env)?);
                            }
                            return self.call_function_value(&func_val, evaled);
                        }
                    }
                }
                let callee = self.eval_expr(fun, env)?;
                if let Value::Function(func_val) = callee {
                    let mut evaled = Vec::with_capacity(args.len());
                    for a in args {
                        evaled.push(self.eval_expr(a, env)?);
                    }
                    return self.call_function_value(&func_val, evaled);
                }
                Err(RuntimeError("Attempted to call a non-function value".to_string()))
            }
            ExprKind::StructInit { .. } => Err(RuntimeError("Struct init not yet supported".to_string())),
            ExprKind::Block(b) => self.eval_block(b, env).map(|v| v.unwrap_or(Value::Unit)),
            ExprKind::If { cond, then_block, else_block } => {
                let c = self.eval_expr(cond, env)?;
                if is_truthy(&c) {
                    self.eval_block(then_block, env).map(|v| v.unwrap_or(Value::Unit))
                } else if let Some(else_expr) = else_block {
                    self.eval_expr(else_expr, env)
                } else {
                    Ok(Value::Unit)
                }
            }
            ExprKind::Match { .. } => Err(RuntimeError("Match not yet supported".to_string())),
            ExprKind::While { cond, body } => {
                while is_truthy(&self.eval_expr(cond, env)?) {
                    if let Some(v) = self.eval_block(body, env)? {
                        return Ok(v);
                    }
                }
                Ok(Value::Unit)
            }
            ExprKind::Perform { .. } => Err(RuntimeError("Effects/perform not yet supported".to_string())),
            ExprKind::Handle { .. } => Err(RuntimeError("Handlers/handle not yet supported".to_string())),
            ExprKind::FnLiteral(f) => {
                let func_val = FunctionValue {
                    params: f.params.clone(),
                    body: f.body.clone(),
                    captured: env.scopes.clone(),
                };
                Ok(Value::Function(func_val))
            }
            ExprKind::Cast { expr: inner } => self.eval_expr(inner, env),
            ExprKind::Error => Err(RuntimeError("Encountered error expression in HIR".to_string())),
        }
    }

    fn eval_literal(&self, pty: &hir::PrimitiveTy, text: &str) -> Result<Value> {
        match pty {
            hir::PrimitiveTy::Bool => Ok(Value::Bool(match text {
                "true" => true,
                "false" => false,
                other => return Err(RuntimeError(format!("Invalid bool literal: {}", other))),
            })),
            hir::PrimitiveTy::Byte => {
                let v: u8 = text.parse().map_err(|_| RuntimeError("Invalid byte literal".to_string()))?;
                Ok(Value::Byte(v))
            }
            hir::PrimitiveTy::I32 => {
                let v: i32 = text.parse().map_err(|_| RuntimeError("Invalid i32 literal".to_string()))?;
                Ok(Value::I32(v))
            }
            hir::PrimitiveTy::I64 => {
                let v: i64 = text.parse().map_err(|_| RuntimeError("Invalid i64 literal".to_string()))?;
                Ok(Value::I64(v))
            }
            hir::PrimitiveTy::F64 => {
                let v: f64 = text.parse().map_err(|_| RuntimeError("Invalid f64 literal".to_string()))?;
                Ok(Value::F64(v))
            }
            hir::PrimitiveTy::Str => Ok(Value::Str(text.to_string())),
        }
    }

    fn apply_unary(&self, op: UnaryOp, v: Value) -> Result<Value> {
        match (op, v) {
            (UnaryOp::Negate, Value::I32(x)) => Ok(Value::I32(-x)),
            (UnaryOp::Negate, Value::I64(x)) => Ok(Value::I64(-x)),
            (UnaryOp::Negate, Value::F64(x)) => Ok(Value::F64(-x)),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            _ => Err(RuntimeError("Invalid unary operation".to_string())),
        }
    }

    fn apply_binary(&self, op: BinaryOp, lv: Value, rv: Value) -> Result<Value> {
        use Value as V;
        match (op, lv, rv) {
            (BinaryOp::Add, V::I32(a), V::I32(b)) => Ok(V::I32(a + b)),
            (BinaryOp::Add, V::I64(a), V::I64(b)) => Ok(V::I64(a + b)),
            (BinaryOp::Add, V::F64(a), V::F64(b)) => Ok(V::F64(a + b)),
            (BinaryOp::Sub, V::I32(a), V::I32(b)) => Ok(V::I32(a - b)),
            (BinaryOp::Sub, V::I64(a), V::I64(b)) => Ok(V::I64(a - b)),
            (BinaryOp::Sub, V::F64(a), V::F64(b)) => Ok(V::F64(a - b)),
            (BinaryOp::Mul, V::I32(a), V::I32(b)) => Ok(V::I32(a * b)),
            (BinaryOp::Mul, V::I64(a), V::I64(b)) => Ok(V::I64(a * b)),
            (BinaryOp::Mul, V::F64(a), V::F64(b)) => Ok(V::F64(a * b)),
            (BinaryOp::Div, V::I32(a), V::I32(b)) => Ok(V::I32(a / b)),
            (BinaryOp::Div, V::I64(a), V::I64(b)) => Ok(V::I64(a / b)),
            (BinaryOp::Div, V::F64(a), V::F64(b)) => Ok(V::F64(a / b)),
            (BinaryOp::Mod, V::I32(a), V::I32(b)) => Ok(V::I32(a % b)),
            (BinaryOp::Mod, V::I64(a), V::I64(b)) => Ok(V::I64(a % b)),
            (BinaryOp::Eq, a, b) => Ok(V::Bool(a == b)),
            (BinaryOp::Ne, a, b) => Ok(V::Bool(a != b)),
            (BinaryOp::Lt, V::I32(a), V::I32(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::I64(a), V::I64(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::F64(a), V::F64(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lte, V::I32(a), V::I32(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::I64(a), V::I64(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::F64(a), V::F64(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Gt, V::I32(a), V::I32(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::I64(a), V::I64(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::F64(a), V::F64(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gte, V::I32(a), V::I32(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::I64(a), V::I64(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::F64(a), V::F64(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::And, V::Bool(a), V::Bool(b)) => Ok(V::Bool(a && b)),
            (BinaryOp::Or, V::Bool(a), V::Bool(b)) => Ok(V::Bool(a || b)),
            _ => Err(RuntimeError("Invalid binary operation or operand types".to_string())),
        }
    }

    // Minimal builtins for runtime-only host functions (std::runtime::*).
    fn try_builtin_call(&self, name: &str, args: &Vec<Expr>, env: &mut Env) -> Result<Option<Value>> {
        match name {
            // std::runtime::exit(code: i32) -> !
            "exit" => {
                if args.len() != 1 { return Err(RuntimeError("exit expects 1 argument".into())); }
                let code = self.eval_expr(&args[0], env)?;
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
                let stream_val = self.eval_expr(&args[0], env)?;
                let data_val = self.eval_expr(&args[1], env)?;
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
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Unit => false,
        Value::Bool(b) => *b,
        Value::Byte(b) => *b != 0,
        Value::I32(i) => *i != 0,
        Value::I64(i) => *i != 0,
        Value::F64(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Function(_) => true,
    }
}

/// Convenience function to run a HIR program end-to-end.
pub fn run_program(items: &[hir::Item]) -> Result<Value> {
    Interpreter::new(items).run()
}

#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub params: Vec<hir::HirParam>,
    pub body: HirBlock,
    pub captured: Vec<HashMap<String, Value>>, // captured lexical scopes
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value as V;
        match (self, other) {
            (V::Unit, V::Unit) => true,
            (V::Bool(a), V::Bool(b)) => a == b,
            (V::Byte(a), V::Byte(b)) => a == b,
            (V::I32(a), V::I32(b)) => a == b,
            (V::I64(a), V::I64(b)) => a == b,
            (V::F64(a), V::F64(b)) => a == b,
            (V::Str(a), V::Str(b)) => a == b,
            (V::Array(a), V::Array(b)) => a == b,
            // Functions are not comparable for equality in this simple runtime
            (V::Function(_), V::Function(_)) => false,
            _ => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Byte(b) => write!(f, "{}", b),
            Value::I32(i) => write!(f, "{}", i),
            Value::I64(i) => write!(f, "{}", i),
            Value::F64(x) => write!(f, "{}", x),
            Value::Str(s) => write!(f, "\"{}\"", s),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Function(_) => write!(f, "<fn>"),
        }
    }
}

/// Convert a runtime value into a process exit code.
/// Convention: 0 indicates success, non-zero indicates failure.
pub fn value_to_exit_code(value: &Value) -> i32 {
    match value {
        Value::Unit => 0,
        Value::Bool(b) => if *b { 0 } else { 1 },
        Value::Byte(b) => *b as i32,
        Value::I32(i) => *i,
        Value::I64(i) => {
            if *i > i32::MAX as i64 { i32::MAX } else if *i < i32::MIN as i64 { i32::MIN } else { *i as i32 }
        }
        Value::F64(f) => {
            if f.is_nan() { 1 } else { *f as i32 }
        }
        // Strings, arrays, and functions default to success
        Value::Str(_) | Value::Array(_) | Value::Function(_) => 0,
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


