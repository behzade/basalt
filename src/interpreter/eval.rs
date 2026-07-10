use std::cell::RefCell;
use std::collections::HashMap;

use crate::hir::{
    self, BinaryOp, Expr, ExprKind, HirBlock, HirFunction, HirHandlerBody, Stmt, UnaryOp,
};
use crate::hir_index::HirIndex;

use super::env::{Env, Result, RuntimeError};
use super::primitive_ops;
use super::runtime;
use super::stack::{AllocationRegionGuard, StackAllocator, StackFrameGuard};
use super::value::{FunctionValue, HandlerEntry, HandlerValue, Value};

enum Flow<T> {
    Value(T),
    Return(Value),
}

macro_rules! flow_value {
    ($expr:expr) => {
        match $expr? {
            Flow::Value(value) => value,
            Flow::Return(value) => return Ok(Flow::Return(value)),
        }
    };
}

/// A very small tree-walking interpreter over the HIR.
pub struct Interpreter {
    index: HirIndex,
    active_handlers: RefCell<Vec<HandlerValue>>,
    stack: RefCell<StackAllocator>,
}

impl Interpreter {
    pub fn new(items: &[hir::Item]) -> Result<Self> {
        let mut stack = StackAllocator::default();
        for item in items {
            if let hir::Item::Memory(memory) = item {
                stack.define_region(memory.name.clone(), memory.byte_limit, memory.object_limit)?;
            }
        }
        Ok(Interpreter {
            index: HirIndex::from_items(items),
            active_handlers: RefCell::new(vec![]),
            stack: RefCell::new(stack),
        })
    }

    /// Runs the program by looking for a top-level function named `main`.
    /// Returns `Unit` if no `main` is present.
    pub fn run(&self) -> Result<Value> {
        if let Some(main_fn) = self.index.function("main") {
            self.call_function(main_fn, vec![])
        } else {
            Ok(Value::Unit)
        }
    }

    pub(crate) fn call_function(&self, function: &HirFunction, args: Vec<Value>) -> Result<Value> {
        match function.extern_kind {
            Some(hir::HirExternKind::RuntimeIntrinsic) => {
                return runtime::call_runtime_intrinsic(function, args);
            }
            Some(hir::HirExternKind::Foreign) => {
                return Err(RuntimeError(format!(
                    "Foreign extern function '{}' cannot run in the interpreter",
                    function.signature.name
                )));
            }
            None => {}
        }

        if !function.signature.params.is_empty() && function.signature.params.len() != args.len() {
            return Err(RuntimeError(format!(
                "Function '{}' expected {} arguments, got {}",
                function.signature.name,
                function.signature.params.len(),
                args.len()
            )));
        }

        let _frame = StackFrameGuard::push(&self.stack, function.signature.name.clone());
        let mut env = Env::new();
        // bind params
        for (idx, param) in function.signature.params.iter().enumerate() {
            if let Some(arg_val) = args.get(idx).cloned() {
                env.define(param.name.clone(), arg_val, false);
            }
        }

        match self.eval_block(&function.body, &mut env)? {
            Flow::Value(value) | Flow::Return(value) => Ok(value),
        }
    }

    pub(crate) fn call_function_value(
        &self,
        function: &FunctionValue,
        args: Vec<Value>,
    ) -> Result<Value> {
        if function.params.len() != args.len() {
            return Err(RuntimeError(format!(
                "Function literal expected {} arguments, got {}",
                function.params.len(),
                args.len()
            )));
        }
        let _frame = StackFrameGuard::push(&self.stack, "<fn literal>");
        // Recreate environment with captured scopes and a fresh top scope for parameters
        let mut env = Env {
            scopes: function.captured.clone(),
        };
        env.push_scope();
        for (idx, param) in function.params.iter().enumerate() {
            env.define(param.name.clone(), args[idx].clone(), false);
        }
        match self.eval_block(&function.body, &mut env)? {
            Flow::Value(value) | Flow::Return(value) => Ok(value),
        }
    }

    /// Evaluate a block, returning the last expression value or an explicit return value.
    fn eval_block(&self, block: &HirBlock, env: &mut Env) -> Result<Flow<Value>> {
        env.push_scope();
        for stmt in &block.stmts {
            match self.eval_stmt(stmt, env)? {
                Flow::Value(()) => {}
                Flow::Return(value) => {
                    env.pop_scope();
                    return Ok(Flow::Return(value));
                }
            }
        }
        let result = if let Some(last_expr) = &block.last_expr {
            self.eval_expr(last_expr, env)?
        } else {
            Flow::Value(Value::Unit)
        };
        env.pop_scope();
        Ok(result)
    }

    fn eval_stmt(&self, stmt: &Stmt, env: &mut Env) -> Result<Flow<()>> {
        match stmt {
            Stmt::Let {
                name,
                value,
                memory,
                is_mut,
                ..
            } => {
                let v = if let Some(expr) = value {
                    let _region_guard = AllocationRegionGuard::enter(&self.stack, memory.clone());
                    flow_value!(self.eval_expr(expr, env))
                } else {
                    Value::Unit
                };
                env.define(name.clone(), v, *is_mut);
                Ok(Flow::Value(()))
            }
            Stmt::Return { value, .. } => {
                let ret = if let Some(expr) = value {
                    flow_value!(self.eval_expr(expr, env))
                } else {
                    Value::Unit
                };
                Ok(Flow::Return(ret))
            }
            Stmt::Assign { lhs, rhs, .. } => {
                if let ExprKind::Path(path) = &lhs.kind {
                    if let Some(var_name) = path.last() {
                        let new_val = flow_value!(self.eval_expr(rhs, env));
                        env.assign(var_name, new_val)?;
                        return Ok(Flow::Value(()));
                    }
                }
                if let ExprKind::FieldAccess { receiver, field } = &lhs.kind {
                    if let ExprKind::Path(p) = &receiver.kind {
                        if let Some(owner_name) = p.last() {
                            let owner_val = env.get(owner_name).ok_or_else(|| {
                                RuntimeError(format!("Undefined variable: {}", owner_name))
                            })?;
                            if let Value::Struct { path, fields } = owner_val {
                                let mut new_fields = fields.clone();
                                let new_val = flow_value!(self.eval_expr(rhs, env));
                                new_fields.insert(field.clone(), new_val);
                                env.assign(
                                    owner_name,
                                    Value::Struct {
                                        path,
                                        fields: new_fields,
                                    },
                                )?;
                                return Ok(Flow::Value(()));
                            } else {
                                return Err(RuntimeError(
                                    "Field assignment on non-struct value".to_string(),
                                ));
                            }
                        }
                    }
                }
                Err(RuntimeError("Unsupported assignment target".to_string()))
            }
            Stmt::Expr { expr, .. } => {
                flow_value!(self.eval_expr(expr, env));
                Ok(Flow::Value(()))
            }
            Stmt::Error { .. } => Err(RuntimeError(
                "Encountered error statement in HIR".to_string(),
            )),
        }
    }

    fn eval_expr(&self, expr: &Expr, env: &mut Env) -> Result<Flow<Value>> {
        match &expr.kind {
            ExprKind::Literal(pty, text) => self.eval_literal(pty, text).map(Flow::Value),
            ExprKind::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(flow_value!(self.eval_expr(item, env)));
                }
                self.alloc(Value::Array(values)).map(Flow::Value)
            }
            ExprKind::Map(entries) => {
                let mut values = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    values.push((
                        flow_value!(self.eval_expr(key, env)),
                        flow_value!(self.eval_expr(value, env)),
                    ));
                }
                self.alloc(Value::Map(values)).map(Flow::Value)
            }
            ExprKind::Path(path) => {
                // Variable lookup for simple paths
                if let Some(name) = path.last() {
                    if let Some(val) = env.get(name) {
                        return Ok(Flow::Value(val));
                    }
                    if let Some(function) = self.resolved_function(expr, name) {
                        return Ok(Flow::Value(Self::function_value(function)));
                    }
                    if let Some(function) = self.index.function(name) {
                        return Ok(Flow::Value(Self::function_value(function)));
                    }
                }
                Err(RuntimeError(format!("Unknown path: {:?}", path)))
            }
            ExprKind::FieldAccess { receiver, field } => {
                let rv = flow_value!(self.eval_expr(receiver, env));
                match rv {
                    Value::Struct { fields, .. } => {
                        if let Some(v) = fields.get(field) {
                            Ok(Flow::Value(v.clone()))
                        } else {
                            Err(RuntimeError(format!("Unknown field '{}'", field)))
                        }
                    }
                    _ => Err(RuntimeError("Field access on non-struct value".to_string())),
                }
            }
            ExprKind::Unary { op, rhs } => {
                let value = flow_value!(self.eval_expr(rhs, env));
                self.apply_unary(*op, value).map(Flow::Value)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = flow_value!(self.eval_expr(lhs, env));
                let rhs = flow_value!(self.eval_expr(rhs, env));
                self.apply_binary(*op, lhs, rhs).map(Flow::Value)
            }
            ExprKind::Call { fun, args } => {
                // Evaluate callee; support calling top-level and function values
                if let ExprKind::Path(p) = &fun.kind {
                    if let Some(name) = p.last() {
                        // Variable binding holding a function value shadows top-level functions.
                        if let Some(Value::Function(func_val)) = env.get(name) {
                            let mut evaled = Vec::with_capacity(args.len());
                            for a in args {
                                evaled.push(flow_value!(self.eval_expr(a, env)));
                            }
                            return self.call_function_value(&func_val, evaled).map(Flow::Value);
                        }
                        if let Some(f) = self.resolved_function(fun, name) {
                            let mut evaled = Vec::with_capacity(args.len());
                            for a in args {
                                evaled.push(flow_value!(self.eval_expr(a, env)));
                            }
                            return self.call_function(f, evaled).map(Flow::Value);
                        }
                        if let Some(f) = self.index.function(name) {
                            let mut evaled = Vec::with_capacity(args.len());
                            for a in args {
                                evaled.push(flow_value!(self.eval_expr(a, env)));
                            }
                            return self.call_function(f, evaled).map(Flow::Value);
                        }
                        // Builtins: minimal host I/O, only after user-visible bindings.
                        if primitive_ops::contains(name) {
                            let mut evaluated_args = Vec::with_capacity(args.len());
                            for arg in args {
                                evaluated_args.push(flow_value!(self.eval_expr(arg, env)));
                            }
                            if let Some(primitive) =
                                primitive_ops::try_primitive_call(name, &evaluated_args)?
                            {
                                return Ok(Flow::Value(primitive));
                            }
                        }
                        if let hir::Ty::Function { ret_type, .. } = &fun.ty {
                            if let hir::Ty::Adt(hir::AdtTy::Enum {
                                name: enum_name, ..
                            }) = ret_type.as_ref()
                            {
                                if !self.index.contains_function(name) {
                                    let mut fields = HashMap::new();
                                    for (idx, arg) in args.iter().enumerate() {
                                        fields.insert(
                                            idx.to_string(),
                                            flow_value!(self.eval_expr(arg, env)),
                                        );
                                    }
                                    let mut path = enum_name.clone();
                                    path.push(name.clone());
                                    return self
                                        .alloc(Value::EnumVariant { path, fields })
                                        .map(Flow::Value);
                                }
                            }
                        }
                    }
                }
                let callee = flow_value!(self.eval_expr(fun, env));
                if let Value::Function(func_val) = callee {
                    let mut evaled = Vec::with_capacity(args.len());
                    for a in args {
                        evaled.push(flow_value!(self.eval_expr(a, env)));
                    }
                    return self.call_function_value(&func_val, evaled).map(Flow::Value);
                }
                Err(RuntimeError(
                    "Attempted to call a non-function value".to_string(),
                ))
            }
            ExprKind::StructInit { path, fields } => {
                let mut map = std::collections::HashMap::new();
                for (name, expr) in fields {
                    let v = flow_value!(self.eval_expr(expr, env));
                    map.insert(name.clone(), v);
                }
                if let hir::Ty::Adt(hir::AdtTy::Enum { .. }) = &expr.ty {
                    let map = self.canonical_enum_fields(path, map);
                    return self
                        .alloc(Value::EnumVariant {
                            path: path.clone(),
                            fields: map,
                        })
                        .map(Flow::Value);
                }
                self.alloc(Value::Struct {
                    path: path.clone(),
                    fields: map,
                })
                .map(Flow::Value)
            }
            ExprKind::Block(block) => self.eval_block(block, env),
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                let condition = flow_value!(self.eval_expr(cond, env));
                if is_truthy(&condition) {
                    self.eval_block(then_block, env)
                } else if let Some(else_expr) = else_block {
                    self.eval_expr(else_expr, env)
                } else {
                    Ok(Flow::Value(Value::Unit))
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                let value = flow_value!(self.eval_expr(scrutinee, env));
                for (pattern, arm_expr) in arms {
                    if let Some(bindings) = self.match_pattern(pattern, &value)? {
                        let mut arm_env = env.clone();
                        arm_env.push_scope();
                        for (name, value) in bindings {
                            arm_env.define(name, value, false);
                        }
                        let result = self.eval_expr(arm_expr, &mut arm_env);
                        arm_env.pop_scope();
                        return result;
                    }
                }
                Err(RuntimeError("Non-exhaustive match at runtime".to_string()))
            }
            ExprKind::While { cond, body } => {
                loop {
                    let condition = flow_value!(self.eval_expr(cond, env));
                    if !is_truthy(&condition) {
                        break;
                    }
                    if let Flow::Return(value) = self.eval_block(body, env)? {
                        return Ok(Flow::Return(value));
                    }
                }
                Ok(Flow::Value(Value::Unit))
            }
            ExprKind::Perform { path, args } => {
                let mut evaled = Vec::with_capacity(args.len());
                for arg in args {
                    evaled.push(flow_value!(self.eval_expr(arg, env)));
                }
                self.perform(path, evaled).map(Flow::Value)
            }
            ExprKind::Handler(handler) => self.eval_handler_body(handler, env).map(Flow::Value),
            ExprKind::Handle { body, handler } => {
                let handler = flow_value!(self.eval_expr(handler, env));
                let Value::Handler(handler) = handler else {
                    return Err(RuntimeError("handle expected handler value".to_string()));
                };
                self.active_handlers.borrow_mut().push(handler);
                let result = self.eval_block(body, env);
                let _ = self.active_handlers.borrow_mut().pop();
                result
            }
            ExprKind::FnLiteral(f) => {
                let func_val = FunctionValue {
                    params: f.params.clone(),
                    body: f.body.clone(),
                    captured: env.scopes.clone(),
                };
                self.alloc(Value::Function(func_val)).map(Flow::Value)
            }
            ExprKind::Cast { expr: inner } => {
                let value = flow_value!(self.eval_expr(inner, env));
                self.cast_value(value, &expr.ty).map(Flow::Value)
            }
            ExprKind::Error => Err(RuntimeError(
                "Encountered error expression in HIR".to_string(),
            )),
        }
    }

    fn perform(&self, path: &[String], args: Vec<Value>) -> Result<Value> {
        if path.len() != 2 {
            return Err(RuntimeError(format!(
                "perform path must be effect-qualified: {:?}",
                path
            )));
        }
        let op_name = &path[1];
        let effect_name = &path[0];
        let function = {
            let active_handlers = self.active_handlers.borrow();
            let mut function = None;
            'search: for handler in active_handlers.iter().rev() {
                for entry in &handler.entries {
                    if !entry.effects.iter().any(|effect| {
                        matches!(
                            effect,
                            hir::Ty::Adt(hir::AdtTy::Effect { name, .. })
                                if name.last().map(|name| name == effect_name).unwrap_or(false)
                        )
                    }) {
                        continue;
                    }
                    if let Some(found) = entry
                        .functions
                        .iter()
                        .find(|function| &function.signature.name == op_name)
                    {
                        function = Some(found.clone());
                        break 'search;
                    }
                }
            }
            function
        };
        if let Some(function) = function {
            return self.call_function(&function, args);
        }
        Err(RuntimeError(format!(
            "Unhandled effect operation {}",
            path.join(".")
        )))
    }

    fn eval_handler_body(&self, handler: &HirHandlerBody, env: &mut Env) -> Result<Value> {
        match handler {
            HirHandlerBody::Path(path) => {
                let Some(name) = path.last() else {
                    return Err(RuntimeError("Empty handler path".to_string()));
                };
                if let Some(Value::Handler(value)) = env.get(name) {
                    return Ok(Value::Handler(value));
                }
                let Some(handler) = self.index.handler(name) else {
                    return Err(RuntimeError(format!("Unknown handler: {}", name)));
                };
                self.alloc(Value::Handler(HandlerValue {
                    entries: vec![HandlerEntry {
                        effects: handler.effects.clone(),
                        functions: handler.functions.clone(),
                    }],
                }))
            }
            HirHandlerBody::Composed { base, handlers } => {
                let mut base = match self.eval_handler_body(base, env)? {
                    Value::Handler(handler) => handler,
                    _ => return Err(RuntimeError("Composed base is not a handler".to_string())),
                };
                for handler in handlers {
                    let Value::Handler(handler) = self.eval_handler_body(handler, env)? else {
                        return Err(RuntimeError(
                            "Composed dependency is not a handler".to_string(),
                        ));
                    };
                    base.entries.extend(handler.entries);
                }
                self.alloc(Value::Handler(base))
            }
            HirHandlerBody::Inline(functions) => self.alloc(Value::Handler(HandlerValue {
                entries: vec![HandlerEntry {
                    effects: vec![],
                    functions: functions.clone(),
                }],
            })),
        }
    }

    fn match_pattern(
        &self,
        pattern: &hir::HirPattern,
        value: &Value,
    ) -> Result<Option<HashMap<String, Value>>> {
        match &pattern.kind {
            hir::HirPatternKind::Wildcard => Ok(Some(HashMap::new())),
            hir::HirPatternKind::Identifier(name) => {
                let mut bindings = HashMap::new();
                bindings.insert(name.clone(), value.clone());
                Ok(Some(bindings))
            }
            hir::HirPatternKind::Literal(pty, text) => {
                let literal = self.eval_literal(pty, text)?;
                if &literal == value {
                    Ok(Some(HashMap::new()))
                } else {
                    Ok(None)
                }
            }
            hir::HirPatternKind::Path { path, args } => {
                let Value::EnumVariant {
                    path: value_path,
                    fields,
                } = value
                else {
                    return Ok(None);
                };

                if path != value_path {
                    return Ok(None);
                }

                let mut bindings = HashMap::new();
                for arg in args {
                    match &arg.kind {
                        hir::HirPatternKind::Identifier(name) => {
                            let bound = fields.get("0").cloned().unwrap_or_else(|| value.clone());
                            bindings.insert(name.clone(), bound);
                        }
                        hir::HirPatternKind::Wildcard => {}
                        _ => {
                            return Err(RuntimeError(
                                "Nested path match patterns are not yet supported".to_string(),
                            ));
                        }
                    }
                }
                Ok(Some(bindings))
            }
        }
    }

    fn resolved_function<'a>(&'a self, fun: &hir::Expr, name: &str) -> Option<&'a HirFunction> {
        let Some(hir::Resolution::Function { defined_in, .. }) = &fun.resolution else {
            return None;
        };
        self.index.resolved_function(defined_in, name)
    }

    fn function_value(function: &HirFunction) -> Value {
        Value::Function(FunctionValue {
            params: function.signature.params.clone(),
            body: function.body.clone(),
            captured: vec![],
        })
    }

    fn canonical_enum_fields(
        &self,
        path: &[String],
        fields: HashMap<String, Value>,
    ) -> HashMap<String, Value> {
        if fields.contains_key("0") {
            return fields;
        }

        let Some(Some(payload)) = self.index.enum_variant_payload(path) else {
            return fields;
        };
        let [
            hir::Ty::Adt(hir::AdtTy::Struct {
                name: payload_struct,
                ..
            }),
        ] = payload.as_slice()
        else {
            return fields;
        };

        let mut wrapped = HashMap::new();
        wrapped.insert(
            "0".to_string(),
            Value::Struct {
                path: payload_struct.clone(),
                fields,
            },
        );
        wrapped
    }

    fn eval_literal(&self, pty: &hir::PrimitiveTy, text: &str) -> Result<Value> {
        match pty {
            hir::PrimitiveTy::Bool => Ok(Value::Bool(match text {
                "true" => true,
                "false" => false,
                other => return Err(RuntimeError(format!("Invalid bool literal: {}", other))),
            })),
            hir::PrimitiveTy::Byte => {
                let v: u8 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid byte literal".to_string()))?;
                Ok(Value::Byte(v))
            }
            hir::PrimitiveTy::I8 => {
                let v: i8 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid i8 literal".to_string()))?;
                Ok(Value::I8(v))
            }
            hir::PrimitiveTy::I16 => {
                let v: i16 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid i16 literal".to_string()))?;
                Ok(Value::I16(v))
            }
            hir::PrimitiveTy::I32 => {
                let v: i32 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid i32 literal".to_string()))?;
                Ok(Value::I32(v))
            }
            hir::PrimitiveTy::I64 => {
                let v: i64 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid i64 literal".to_string()))?;
                Ok(Value::I64(v))
            }
            hir::PrimitiveTy::U8 => {
                let v: u8 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid u8 literal".to_string()))?;
                Ok(Value::U8(v))
            }
            hir::PrimitiveTy::U16 => {
                let v: u16 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid u16 literal".to_string()))?;
                Ok(Value::U16(v))
            }
            hir::PrimitiveTy::U32 => {
                let v: u32 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid u32 literal".to_string()))?;
                Ok(Value::U32(v))
            }
            hir::PrimitiveTy::U64 => {
                let v: u64 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid u64 literal".to_string()))?;
                Ok(Value::U64(v))
            }
            hir::PrimitiveTy::F32 => {
                let v: f32 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid f32 literal".to_string()))?;
                Ok(Value::F32(v))
            }
            hir::PrimitiveTy::F64 => {
                let v: f64 = text
                    .parse()
                    .map_err(|_| RuntimeError("Invalid f64 literal".to_string()))?;
                Ok(Value::F64(v))
            }
            hir::PrimitiveTy::Str => self.alloc(Value::Str(text.to_string())),
        }
    }

    fn alloc(&self, value: Value) -> Result<Value> {
        self.stack.borrow_mut().alloc_value(value)
    }

    fn cast_value(&self, value: Value, target_ty: &hir::Ty) -> Result<Value> {
        let hir::Ty::Primitive(target) = target_ty else {
            return Ok(value);
        };

        use hir::PrimitiveTy as P;
        match target {
            P::Bool => match value {
                Value::Bool(b) => Ok(Value::Bool(b)),
                other => Err(RuntimeError(format!("Cannot cast {} to bool", other))),
            },
            P::Byte => Ok(Value::Byte(Self::checked_unsigned_cast::<u8>(
                &value, "byte",
            )?)),
            P::U8 => Ok(Value::U8(Self::checked_unsigned_cast::<u8>(&value, "u8")?)),
            P::I8 => Ok(Value::I8(Self::checked_signed_cast::<i8>(&value, "i8")?)),
            P::I16 => Ok(Value::I16(Self::checked_signed_cast::<i16>(&value, "i16")?)),
            P::I32 => Ok(Value::I32(Self::checked_signed_cast::<i32>(&value, "i32")?)),
            P::I64 => Ok(Value::I64(Self::checked_signed_cast::<i64>(&value, "i64")?)),
            P::U16 => Ok(Value::U16(Self::checked_unsigned_cast::<u16>(
                &value, "u16",
            )?)),
            P::U32 => Ok(Value::U32(Self::checked_unsigned_cast::<u32>(
                &value, "u32",
            )?)),
            P::U64 => Ok(Value::U64(Self::checked_unsigned_cast::<u64>(
                &value, "u64",
            )?)),
            P::F32 => Ok(Value::F32(Self::value_to_f64(&value)? as f32)),
            P::F64 => Ok(Value::F64(Self::value_to_f64(&value)?)),
            P::Str => match value {
                Value::Str(s) => Ok(Value::Str(s)),
                other => Err(RuntimeError(format!("Cannot cast {} to str", other))),
            },
        }
    }

    fn checked_signed_cast<T>(value: &Value, target: &str) -> Result<T>
    where
        T: TryFrom<i128>,
    {
        let value = Self::value_to_i128(value)?;
        T::try_from(value).map_err(|_| RuntimeError(format!("value does not fit {}", target)))
    }

    fn checked_unsigned_cast<T>(value: &Value, target: &str) -> Result<T>
    where
        T: TryFrom<u128>,
    {
        let value = Self::value_to_u128(value)?;
        T::try_from(value).map_err(|_| RuntimeError(format!("value does not fit {}", target)))
    }

    fn value_to_i128(value: &Value) -> Result<i128> {
        match value {
            Value::Byte(v) | Value::U8(v) => Ok(*v as i128),
            Value::I8(v) => Ok(*v as i128),
            Value::I16(v) => Ok(*v as i128),
            Value::I32(v) => Ok(*v as i128),
            Value::I64(v) => Ok(*v as i128),
            Value::U16(v) => Ok(*v as i128),
            Value::U32(v) => Ok(*v as i128),
            Value::U64(v) => i128::try_from(*v)
                .map_err(|_| RuntimeError("u64 value does not fit signed integer".to_string())),
            Value::F32(v) => Ok(*v as i128),
            Value::F64(v) => Ok(*v as i128),
            other => Err(RuntimeError(format!("Cannot cast {} to integer", other))),
        }
    }

    fn value_to_u128(value: &Value) -> Result<u128> {
        match value {
            Value::Byte(v) | Value::U8(v) => Ok(*v as u128),
            Value::I8(v) => u128::try_from(*v).map_err(|_| {
                RuntimeError("negative value does not fit unsigned integer".to_string())
            }),
            Value::I16(v) => u128::try_from(*v).map_err(|_| {
                RuntimeError("negative value does not fit unsigned integer".to_string())
            }),
            Value::I32(v) => u128::try_from(*v).map_err(|_| {
                RuntimeError("negative value does not fit unsigned integer".to_string())
            }),
            Value::I64(v) => u128::try_from(*v).map_err(|_| {
                RuntimeError("negative value does not fit unsigned integer".to_string())
            }),
            Value::U16(v) => Ok(*v as u128),
            Value::U32(v) => Ok(*v as u128),
            Value::U64(v) => Ok(*v as u128),
            Value::F32(v) if *v >= 0.0 => Ok(*v as u128),
            Value::F64(v) if *v >= 0.0 => Ok(*v as u128),
            other => Err(RuntimeError(format!(
                "Cannot cast {} to unsigned integer",
                other
            ))),
        }
    }

    fn value_to_f64(value: &Value) -> Result<f64> {
        match value {
            Value::Byte(v) | Value::U8(v) => Ok(*v as f64),
            Value::I8(v) => Ok(*v as f64),
            Value::I16(v) => Ok(*v as f64),
            Value::I32(v) => Ok(*v as f64),
            Value::I64(v) => Ok(*v as f64),
            Value::U16(v) => Ok(*v as f64),
            Value::U32(v) => Ok(*v as f64),
            Value::U64(v) => Ok(*v as f64),
            Value::F32(v) => Ok(*v as f64),
            Value::F64(v) => Ok(*v),
            other => Err(RuntimeError(format!("Cannot cast {} to float", other))),
        }
    }

    fn apply_unary(&self, op: UnaryOp, v: Value) -> Result<Value> {
        match (op, v) {
            (UnaryOp::Negate, Value::I8(x)) => Ok(Value::I8(-x)),
            (UnaryOp::Negate, Value::I16(x)) => Ok(Value::I16(-x)),
            (UnaryOp::Negate, Value::I32(x)) => Ok(Value::I32(-x)),
            (UnaryOp::Negate, Value::I64(x)) => Ok(Value::I64(-x)),
            (UnaryOp::Negate, Value::F32(x)) => Ok(Value::F32(-x)),
            (UnaryOp::Negate, Value::F64(x)) => Ok(Value::F64(-x)),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            _ => Err(RuntimeError("Invalid unary operation".to_string())),
        }
    }

    fn apply_binary(&self, op: BinaryOp, lv: Value, rv: Value) -> Result<Value> {
        use Value as V;
        match (op, lv, rv) {
            (BinaryOp::Add, V::I8(a), V::I8(b)) => Ok(V::I8(a + b)),
            (BinaryOp::Add, V::I16(a), V::I16(b)) => Ok(V::I16(a + b)),
            (BinaryOp::Add, V::I32(a), V::I32(b)) => Ok(V::I32(a + b)),
            (BinaryOp::Add, V::I64(a), V::I64(b)) => Ok(V::I64(a + b)),
            (BinaryOp::Add, V::U8(a), V::U8(b)) => Ok(V::U8(a + b)),
            (BinaryOp::Add, V::U16(a), V::U16(b)) => Ok(V::U16(a + b)),
            (BinaryOp::Add, V::U32(a), V::U32(b)) => Ok(V::U32(a + b)),
            (BinaryOp::Add, V::U64(a), V::U64(b)) => Ok(V::U64(a + b)),
            (BinaryOp::Add, V::F32(a), V::F32(b)) => Ok(V::F32(a + b)),
            (BinaryOp::Add, V::F64(a), V::F64(b)) => Ok(V::F64(a + b)),
            (BinaryOp::Sub, V::I8(a), V::I8(b)) => Ok(V::I8(a - b)),
            (BinaryOp::Sub, V::I16(a), V::I16(b)) => Ok(V::I16(a - b)),
            (BinaryOp::Sub, V::I32(a), V::I32(b)) => Ok(V::I32(a - b)),
            (BinaryOp::Sub, V::I64(a), V::I64(b)) => Ok(V::I64(a - b)),
            (BinaryOp::Sub, V::U8(a), V::U8(b)) => Ok(V::U8(a - b)),
            (BinaryOp::Sub, V::U16(a), V::U16(b)) => Ok(V::U16(a - b)),
            (BinaryOp::Sub, V::U32(a), V::U32(b)) => Ok(V::U32(a - b)),
            (BinaryOp::Sub, V::U64(a), V::U64(b)) => Ok(V::U64(a - b)),
            (BinaryOp::Sub, V::F32(a), V::F32(b)) => Ok(V::F32(a - b)),
            (BinaryOp::Sub, V::F64(a), V::F64(b)) => Ok(V::F64(a - b)),
            (BinaryOp::Mul, V::I8(a), V::I8(b)) => Ok(V::I8(a * b)),
            (BinaryOp::Mul, V::I16(a), V::I16(b)) => Ok(V::I16(a * b)),
            (BinaryOp::Mul, V::I32(a), V::I32(b)) => Ok(V::I32(a * b)),
            (BinaryOp::Mul, V::I64(a), V::I64(b)) => Ok(V::I64(a * b)),
            (BinaryOp::Mul, V::U8(a), V::U8(b)) => Ok(V::U8(a * b)),
            (BinaryOp::Mul, V::U16(a), V::U16(b)) => Ok(V::U16(a * b)),
            (BinaryOp::Mul, V::U32(a), V::U32(b)) => Ok(V::U32(a * b)),
            (BinaryOp::Mul, V::U64(a), V::U64(b)) => Ok(V::U64(a * b)),
            (BinaryOp::Mul, V::F32(a), V::F32(b)) => Ok(V::F32(a * b)),
            (BinaryOp::Mul, V::F64(a), V::F64(b)) => Ok(V::F64(a * b)),
            (BinaryOp::Div, V::I8(a), V::I8(b)) => Ok(V::I8(a / b)),
            (BinaryOp::Div, V::I16(a), V::I16(b)) => Ok(V::I16(a / b)),
            (BinaryOp::Div, V::I32(a), V::I32(b)) => Ok(V::I32(a / b)),
            (BinaryOp::Div, V::I64(a), V::I64(b)) => Ok(V::I64(a / b)),
            (BinaryOp::Div, V::U8(a), V::U8(b)) => Ok(V::U8(a / b)),
            (BinaryOp::Div, V::U16(a), V::U16(b)) => Ok(V::U16(a / b)),
            (BinaryOp::Div, V::U32(a), V::U32(b)) => Ok(V::U32(a / b)),
            (BinaryOp::Div, V::U64(a), V::U64(b)) => Ok(V::U64(a / b)),
            (BinaryOp::Div, V::F32(a), V::F32(b)) => Ok(V::F32(a / b)),
            (BinaryOp::Div, V::F64(a), V::F64(b)) => Ok(V::F64(a / b)),
            (BinaryOp::Mod, V::I8(a), V::I8(b)) => Ok(V::I8(a % b)),
            (BinaryOp::Mod, V::I16(a), V::I16(b)) => Ok(V::I16(a % b)),
            (BinaryOp::Mod, V::I32(a), V::I32(b)) => Ok(V::I32(a % b)),
            (BinaryOp::Mod, V::I64(a), V::I64(b)) => Ok(V::I64(a % b)),
            (BinaryOp::Mod, V::U8(a), V::U8(b)) => Ok(V::U8(a % b)),
            (BinaryOp::Mod, V::U16(a), V::U16(b)) => Ok(V::U16(a % b)),
            (BinaryOp::Mod, V::U32(a), V::U32(b)) => Ok(V::U32(a % b)),
            (BinaryOp::Mod, V::U64(a), V::U64(b)) => Ok(V::U64(a % b)),
            (BinaryOp::Eq, a, b) => Ok(V::Bool(a == b)),
            (BinaryOp::Ne, a, b) => Ok(V::Bool(a != b)),
            (BinaryOp::Lt, V::I8(a), V::I8(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::I16(a), V::I16(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::I32(a), V::I32(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::I64(a), V::I64(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::U8(a), V::U8(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::U16(a), V::U16(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::U32(a), V::U32(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::U64(a), V::U64(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::F32(a), V::F32(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lt, V::F64(a), V::F64(b)) => Ok(V::Bool(a < b)),
            (BinaryOp::Lte, V::I8(a), V::I8(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::I16(a), V::I16(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::I32(a), V::I32(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::I64(a), V::I64(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::U8(a), V::U8(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::U16(a), V::U16(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::U32(a), V::U32(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::U64(a), V::U64(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::F32(a), V::F32(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Lte, V::F64(a), V::F64(b)) => Ok(V::Bool(a <= b)),
            (BinaryOp::Gt, V::I8(a), V::I8(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::I16(a), V::I16(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::I32(a), V::I32(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::I64(a), V::I64(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::U8(a), V::U8(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::U16(a), V::U16(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::U32(a), V::U32(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::U64(a), V::U64(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::F32(a), V::F32(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gt, V::F64(a), V::F64(b)) => Ok(V::Bool(a > b)),
            (BinaryOp::Gte, V::I8(a), V::I8(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::I16(a), V::I16(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::I32(a), V::I32(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::I64(a), V::I64(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::U8(a), V::U8(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::U16(a), V::U16(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::U32(a), V::U32(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::U64(a), V::U64(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::F32(a), V::F32(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::Gte, V::F64(a), V::F64(b)) => Ok(V::Bool(a >= b)),
            (BinaryOp::And, V::Bool(a), V::Bool(b)) => Ok(V::Bool(a && b)),
            (BinaryOp::Or, V::Bool(a), V::Bool(b)) => Ok(V::Bool(a || b)),
            _ => Err(RuntimeError(
                "Invalid binary operation or operand types".to_string(),
            )),
        }
    }
}

/// Convenience function to run a HIR program end-to-end.
pub fn run_program(items: &[hir::Item]) -> Result<Value> {
    Interpreter::new(items)?.run()
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Unit => false,
        Value::Bool(b) => *b,
        Value::Byte(b) => *b != 0,
        Value::I8(i) => *i != 0,
        Value::I16(i) => *i != 0,
        Value::I32(i) => *i != 0,
        Value::I64(i) => *i != 0,
        Value::U8(i) => *i != 0,
        Value::U16(i) => *i != 0,
        Value::U32(i) => *i != 0,
        Value::U64(i) => *i != 0,
        Value::F32(f) => *f != 0.0,
        Value::F64(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Map(m) => !m.is_empty(),
        Value::Struct { .. } => true,
        Value::EnumVariant { .. } => true,
        Value::Function(_) => true,
        Value::Handler(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::span::Span;

    use crate::token::SimpleSpan;

    fn span() -> SimpleSpan {
        SimpleSpan::new((), 0..0)
    }

    fn i32_ty() -> hir::Ty {
        hir::Ty::Primitive(hir::PrimitiveTy::I32)
    }

    fn literal_i32(value: &str) -> Expr {
        Expr {
            kind: ExprKind::Literal(hir::PrimitiveTy::I32, value.to_string()),
            ty: i32_ty(),
            span: span(),
            resolution: None,
        }
    }

    fn perform_value(effect: &str) -> Expr {
        Expr {
            kind: ExprKind::Perform {
                path: vec![effect.to_string(), "value".to_string()],
                args: vec![],
            },
            ty: i32_ty(),
            span: span(),
            resolution: None,
        }
    }

    fn str_ty() -> hir::Ty {
        hir::Ty::Primitive(hir::PrimitiveTy::Str)
    }

    fn struct_ty(name: &str) -> hir::Ty {
        hir::Ty::Adt(hir::AdtTy::Struct {
            name: vec![name.to_string()],
            generics: vec![],
        })
    }

    fn enum_ty(name: &str) -> hir::Ty {
        hir::Ty::Adt(hir::AdtTy::Enum {
            name: vec![name.to_string()],
            generics: vec![],
        })
    }

    fn enum_variant_expr(enum_name: &str, variant_name: &str) -> Expr {
        Expr {
            kind: ExprKind::StructInit {
                path: vec![enum_name.to_string(), variant_name.to_string()],
                fields: vec![],
            },
            ty: enum_ty(enum_name),
            span: span(),
            resolution: None,
        }
    }

    fn function_ty(ret_type: hir::Ty) -> hir::Ty {
        hir::Ty::Function {
            param_types: vec![],
            ret_type: Box::new(ret_type),
            effects: vec![],
        }
    }

    fn resolved_function_path(name: &str, defined_in: &str, ret_type: hir::Ty) -> Expr {
        Expr {
            kind: ExprKind::Path(vec![name.to_string()]),
            ty: function_ty(ret_type),
            span: span(),
            resolution: Some(hir::Resolution::Function {
                defined_in: defined_in.into(),
                span: span(),
            }),
        }
    }

    fn map_ty(key: hir::PrimitiveTy, value: hir::Ty) -> hir::Ty {
        hir::Ty::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    fn function(name: &str, body_expr: Expr) -> HirFunction {
        function_in(name, "test.bst", body_expr)
    }

    fn function_in(name: &str, defined_in: &str, body_expr: Expr) -> HirFunction {
        HirFunction {
            signature: hir::HirFunctionSignature {
                name: name.to_string(),
                params: vec![],
                ret_type: i32_ty(),
                effects: vec![],
            },
            body: HirBlock {
                stmts: vec![],
                last_expr: Some(Box::new(body_expr)),
                ty: i32_ty(),
            },
            is_public: false,
            extern_kind: None,
            defined_in: defined_in.into(),
            span: span(),
            context_id: None,
        }
    }

    fn main_with_body(stmts: Vec<Stmt>, body_expr: Expr) -> HirFunction {
        HirFunction {
            signature: hir::HirFunctionSignature {
                name: "main".to_string(),
                params: vec![],
                ret_type: body_expr.ty.clone(),
                effects: vec![],
            },
            body: HirBlock {
                stmts,
                last_expr: Some(Box::new(body_expr.clone())),
                ty: body_expr.ty,
            },
            is_public: false,
            extern_kind: None,
            defined_in: "test.bst".into(),
            span: span(),
            context_id: None,
        }
    }

    fn enum_item(enum_name: &str, variant_name: &str, payload: Option<Vec<hir::Ty>>) -> hir::Item {
        hir::Item::Enum(hir::HirEnumDef {
            name: enum_name.to_string(),
            variants: vec![hir::HirEnumVariant {
                name: variant_name.to_string(),
                payload,
                name_span: Some(span()),
            }],
            is_public: false,
            defined_in: "test.bst".into(),
            span: span(),
            context_id: None,
        })
    }

    fn effect_ty(name: &str) -> hir::Ty {
        hir::Ty::Adt(hir::AdtTy::Effect {
            name: vec![name.to_string()],
            generics: vec![],
        })
    }

    fn handler(name: &str, effect: &str, method_value: i32) -> hir::Item {
        hir::Item::Handler(hir::HirHandlerDef {
            name: name.to_string(),
            effects: vec![effect_ty(effect)],
            functions: vec![function("value", literal_i32(&method_value.to_string()))],
            is_public: false,
            defined_in: "test.bst".into(),
            span: span(),
        })
    }

    fn main_with_handler(handler_body: HirHandlerBody, body_expr: Expr) -> hir::Item {
        hir::Item::Fn(function(
            "main",
            Expr {
                kind: ExprKind::Handle {
                    body: HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(body_expr)),
                        ty: i32_ty(),
                    },
                    handler: Box::new(Expr {
                        kind: ExprKind::Handler(handler_body),
                        ty: hir::Ty::Handler { effects: vec![] },
                        span: span(),
                        resolution: None,
                    }),
                },
                ty: i32_ty(),
                span: span(),
                resolution: None,
            },
        ))
    }

    #[test]
    fn handles_perform_with_path_handler() {
        let items = vec![
            handler("AskHandler", "Ask", 7),
            main_with_handler(
                HirHandlerBody::Path(vec!["AskHandler".to_string()]),
                perform_value("Ask"),
            ),
        ];

        assert_eq!(run_program(&items).unwrap(), Value::I32(7));
    }

    #[test]
    fn handles_perform_with_composed_handler() {
        let items = vec![
            handler("BaseHandler", "Base", 1),
            handler("DependencyHandler", "Dependency", 9),
            main_with_handler(
                HirHandlerBody::Composed {
                    base: Box::new(HirHandlerBody::Path(vec!["BaseHandler".to_string()])),
                    handlers: vec![HirHandlerBody::Path(vec!["DependencyHandler".to_string()])],
                },
                perform_value("Dependency"),
            ),
        ];

        assert_eq!(run_program(&items).unwrap(), Value::I32(9));
    }

    #[test]
    fn matches_enum_variant_and_binds_payload() {
        let items = vec![
            enum_item("UserType", "B2B", Some(vec![struct_ty("Company")])),
            hir::Item::Fn(function(
                "main",
                Expr {
                    kind: ExprKind::Match {
                        scrutinee: Box::new(Expr {
                            kind: ExprKind::StructInit {
                                path: vec!["UserType".to_string(), "B2B".to_string()],
                                fields: vec![(
                                    "name".to_string(),
                                    Expr {
                                        kind: ExprKind::Literal(
                                            hir::PrimitiveTy::Str,
                                            "acme".to_string(),
                                        ),
                                        ty: str_ty(),
                                        span: span(),
                                        resolution: None,
                                    },
                                )],
                            },
                            ty: enum_ty("UserType"),
                            span: span(),
                            resolution: None,
                        }),
                        arms: vec![(
                            hir::HirPattern {
                                kind: hir::HirPatternKind::Path {
                                    path: vec!["UserType".to_string(), "B2B".to_string()],
                                    args: vec![hir::HirPattern {
                                        kind: hir::HirPatternKind::Identifier("b2b".to_string()),
                                        ty: struct_ty("Company"),
                                    }],
                                },
                                ty: enum_ty("UserType"),
                            },
                            Expr {
                                kind: ExprKind::FieldAccess {
                                    receiver: Box::new(Expr {
                                        kind: ExprKind::Path(vec!["b2b".to_string()]),
                                        ty: struct_ty("Company"),
                                        span: span(),
                                        resolution: None,
                                    }),
                                    field: "name".to_string(),
                                },
                                ty: str_ty(),
                                span: span(),
                                resolution: None,
                            },
                        )],
                    },
                    ty: str_ty(),
                    span: span(),
                    resolution: None,
                },
            )),
        ];

        assert_eq!(run_program(&items).unwrap(), Value::Str("acme".to_string()));
    }

    #[test]
    fn enum_variant_matching_uses_full_constructor_identity() {
        let items = vec![hir::Item::Fn(function(
            "main",
            Expr {
                kind: ExprKind::Match {
                    scrutinee: Box::new(enum_variant_expr("A", "Same")),
                    arms: vec![
                        (
                            hir::HirPattern {
                                kind: hir::HirPatternKind::Path {
                                    path: vec!["B".to_string(), "Same".to_string()],
                                    args: vec![],
                                },
                                ty: enum_ty("B"),
                            },
                            literal_i32("1"),
                        ),
                        (
                            hir::HirPattern {
                                kind: hir::HirPatternKind::Path {
                                    path: vec!["A".to_string(), "Same".to_string()],
                                    args: vec![],
                                },
                                ty: enum_ty("A"),
                            },
                            literal_i32("2"),
                        ),
                    ],
                },
                ty: i32_ty(),
                span: span(),
                resolution: None,
            },
        ))];

        assert_eq!(run_program(&items).unwrap(), Value::I32(2));
    }

    #[test]
    fn calls_function_selected_by_hir_resolution() {
        let items = vec![
            hir::Item::Fn(function_in("value", "a.bst", literal_i32("1"))),
            hir::Item::Fn(function_in("value", "b.bst", literal_i32("2"))),
            hir::Item::Fn(function(
                "main",
                Expr {
                    kind: ExprKind::Call {
                        fun: Box::new(Expr {
                            kind: ExprKind::Path(vec!["value".to_string()]),
                            ty: hir::Ty::Function {
                                param_types: vec![],
                                ret_type: Box::new(i32_ty()),
                                effects: vec![],
                            },
                            span: span(),
                            resolution: Some(hir::Resolution::Function {
                                defined_in: "a.bst".into(),
                                span: span(),
                            }),
                        }),
                        args: vec![],
                    },
                    ty: i32_ty(),
                    span: span(),
                    resolution: None,
                },
            )),
        ];

        assert_eq!(run_program(&items).unwrap(), Value::I32(1));
    }

    #[test]
    fn resolved_function_path_value_uses_selected_definition() {
        let selected_ty = function_ty(i32_ty());
        let items = vec![
            hir::Item::Fn(function_in("value", "a.bst", literal_i32("1"))),
            hir::Item::Fn(function_in("value", "b.bst", literal_i32("2"))),
            hir::Item::Fn(main_with_body(
                vec![Stmt::Let {
                    name: "selected".to_string(),
                    value: Some(resolved_function_path("value", "a.bst", i32_ty())),
                    ty: selected_ty.clone(),
                    is_mut: false,
                    memory: None,
                    span: span(),
                    name_span: Some(span()),
                }],
                Expr {
                    kind: ExprKind::Call {
                        fun: Box::new(Expr {
                            kind: ExprKind::Path(vec!["selected".to_string()]),
                            ty: selected_ty,
                            span: span(),
                            resolution: None,
                        }),
                        args: vec![],
                    },
                    ty: i32_ty(),
                    span: span(),
                    resolution: None,
                },
            )),
        ];

        assert_eq!(run_program(&items).unwrap(), Value::I32(1));
    }

    #[test]
    fn evaluates_map_literals_and_len() {
        let map_type = map_ty(hir::PrimitiveTy::Str, i32_ty());
        let map_expr = Expr {
            kind: ExprKind::Map(vec![
                (
                    Expr {
                        kind: ExprKind::Literal(hir::PrimitiveTy::Str, "a".to_string()),
                        ty: str_ty(),
                        span: span(),
                        resolution: None,
                    },
                    literal_i32("1"),
                ),
                (
                    Expr {
                        kind: ExprKind::Literal(hir::PrimitiveTy::Str, "b".to_string()),
                        ty: str_ty(),
                        span: span(),
                        resolution: None,
                    },
                    literal_i32("2"),
                ),
            ]),
            ty: map_type,
            span: span(),
            resolution: None,
        };
        let items = vec![hir::Item::Fn(function(
            "main",
            Expr {
                kind: ExprKind::Call {
                    fun: Box::new(Expr {
                        kind: ExprKind::Path(vec!["len".to_string()]),
                        ty: i32_ty(),
                        span: span(),
                        resolution: None,
                    }),
                    args: vec![map_expr],
                },
                ty: i32_ty(),
                span: span(),
                resolution: None,
            },
        ))];

        assert_eq!(run_program(&items).unwrap(), Value::I32(2));
    }
}
