//! src/interpreter.rs
//!
//! A simple interpreter for executing MIR with dynamic effect handler context.

use crate::mir::data::*;
use crate::ast::{Literal, BinaryOp};
use std::collections::HashMap;

/// Represents a value in the interpreter
#[derive(Debug, Clone)]
pub enum Value {
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
    Unit,
}

impl From<Literal<'_>> for Value {
    fn from(lit: Literal<'_>) -> Self {
        match lit {
            Literal::I64(n) => Value::I64(n),
            Literal::F64(n) => Value::F64(n),
            Literal::Bool(b) => Value::Bool(b),
            Literal::Str(s) => Value::String(s.to_string()),
            Literal::Unit => Value::Unit,
        }
    }
}

/// The execution environment for a function
#[derive(Debug)]
pub struct Environment<'src> {
    /// Local variables and their values
    pub locals: HashMap<LocalId, Value>,
    /// Dynamic handler context
    pub handler_context: HandlerContext<'src>,
}

impl<'src> Environment<'src> {
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
            handler_context: HandlerContext::new(),
        }
    }

    pub fn with_handler_context(context: HandlerContext<'src>) -> Self {
        Self {
            locals: HashMap::new(),
            handler_context: context,
        }
    }
}

/// The main interpreter for executing MIR
pub struct Interpreter<'src> {
    program: &'src MirProgram<'src>,
    /// Built-in handlers for effects
    handlers: HashMap<&'src str, Box<dyn Handler<'src>>>,
}

/// Trait for effect handlers
pub trait Handler<'src> {
    fn handle_operation(&self, operation: &str, args: Vec<Value>) -> Value;
}

/// A simple console IO handler for testing
pub struct ConsoleIOHandler;

impl<'src> Handler<'src> for ConsoleIOHandler {
    fn handle_operation(&self, operation: &str, args: Vec<Value>) -> Value {
        match operation {
            "Read" => {
                println!("[IO.Read] Returning mock input");
                Value::String("mock_input".to_string())
            }
            "Write" => {
                if let Some(Value::String(msg)) = args.first() {
                    println!("[IO.Write] {}", msg);
                }
                Value::Unit
            }
            _ => {
                println!("[ConsoleIO] Unknown operation: {}", operation);
                Value::Unit
            }
        }
    }
}

impl<'src> Interpreter<'src> {
    pub fn new(program: &'src MirProgram<'src>) -> Self {
        let mut handlers = HashMap::new();
        handlers.insert("ConsoleIO", Box::new(ConsoleIOHandler) as Box<dyn Handler<'src>>);
        
        Self { program, handlers }
    }

    /// Execute a function by name
    pub fn execute_function(&self, func_name: &str, args: Vec<Value>) -> Result<Value, String> {
        let func = self.program.functions.get(func_name)
            .ok_or_else(|| format!("Function '{}' not found", func_name))?;

        // Create environment with handler context from function
        let mut env = Environment::with_handler_context(func.handler_context.clone());
        
        // Set up parameters
        for (param_id, arg_value) in func.params.iter().zip(args.iter()) {
            env.locals.insert(*param_id, arg_value.clone());
        }

        // Execute the function starting from the first block
        self.execute_basic_block(func, 0, &mut env)
    }

    /// Execute a basic block
    fn execute_basic_block(
        &self,
        func: &'src MirFunction<'src>,
        block_id: BasicBlockId,
        env: &mut Environment<'src>,
    ) -> Result<Value, String> {
        let block = &func.basic_blocks[block_id];

        // Execute all statements in the block
        for stmt in &block.statements {
            self.execute_statement(stmt, func, env)?;
        }

        // Execute the terminator
        self.execute_terminator(&block.terminator, func, env)
    }

    /// Execute a single statement
    fn execute_statement(
        &self,
        stmt: &'src Statement<'src>,
        func: &'src MirFunction<'src>,
        env: &mut Environment<'src>,
    ) -> Result<(), String> {
        match stmt {
            Statement::Assign(place, rvalue) => {
                let value = self.evaluate_rvalue(rvalue, func, env)?;
                env.locals.insert(place.local, value);
                Ok(())
            }
        }
    }

    /// Execute a terminator
    fn execute_terminator(
        &self,
        terminator: &'src Terminator<'src>,
        func: &'src MirFunction<'src>,
        env: &mut Environment<'src>,
    ) -> Result<Value, String> {
        match terminator {
            Terminator::Goto { target } => {
                self.execute_basic_block(func, *target, env)
            }
            Terminator::SwitchInt { discr, targets, otherwise } => {
                let value = self.evaluate_operand(discr, func, env)?;
                let int_value = match value {
                    Value::I64(n) => n as u64,
                    Value::Bool(b) => if b { 1 } else { 0 },
                    _ => return Err("SwitchInt requires integer or boolean operand".to_string()),
                };

                // Find matching target
                for (target_value, target_block) in targets {
                    if int_value == *target_value {
                        return self.execute_basic_block(func, *target_block, env);
                    }
                }

                // Use otherwise target
                self.execute_basic_block(func, *otherwise, env)
            }
            Terminator::Call { func: func_name, args, destination, target } => {
                let arg_values: Vec<Value> = args.iter()
                    .map(|arg| self.evaluate_operand(arg, func, env))
                    .collect::<Result<Vec<_>, _>>()?;

                let result = match *func_name {
                    "println" => {
                        if let Some(Value::String(s)) = arg_values.first() {
                            println!("{}", s);
                        }
                        Value::Unit
                    }
                    _ => {
                        // Call user-defined function with current handler context
                        let func = self.program.functions.get(func_name)
                            .ok_or_else(|| format!("Function '{}' not found", func_name))?;

                        // Create environment with current handler context
                        let mut call_env = Environment::with_handler_context(env.handler_context.clone());
                        
                        // Set up parameters
                        for (param_id, arg_value) in func.params.iter().zip(arg_values.iter()) {
                            call_env.locals.insert(*param_id, arg_value.clone());
                        }

                        // Execute the function starting from the first block
                        self.execute_basic_block(func, 0, &mut call_env)?
                    }
                };

                env.locals.insert(destination.local.clone(), result);
                self.execute_basic_block(func, *target, env)
            }
            Terminator::PushHandler { effect, handler, target } => {
                // Push handler onto the dynamic stack
                env.handler_context.push_handler(effect, handler);
                
                // Execute the target block with the handler in scope
                self.execute_basic_block(func, *target, env)
            }
            Terminator::PopHandler { target } => {
                // Pop the topmost local handler
                env.handler_context.pop_local_handler();
                
                // Execute the target block
                self.execute_basic_block(func, *target, env)
            }
            Terminator::Perform { effect, operation, args, destination, continuation, no_handler } => {
                // Look up handler dynamically
                if let Some(handler_name) = env.handler_context.find_handler(effect) {
                    // Evaluate arguments
                    let arg_values: Vec<Value> = args.iter()
                        .map(|arg| self.evaluate_operand(arg, func, env))
                        .collect::<Result<Vec<_>, _>>()?;

                    // Get the handler
                    let handler = self.handlers.get(handler_name)
                        .ok_or_else(|| format!("Handler '{}' not found", handler_name))?;

                    // Execute the handler
                    let result = handler.handle_operation(operation, arg_values);
                    
                    // Store result and continue
                    env.locals.insert(destination.local.clone(), result);
                    self.execute_basic_block(func, *continuation, env)
                } else {
                    // No handler found, execute no_handler block
                    self.execute_basic_block(func, *no_handler, env)
                }
            }
            Terminator::Resume { value, target } => {
                // For now, just evaluate the value and continue
                let result = self.evaluate_operand(value, func, env)?;
                // In a real implementation, this would restore a captured continuation
                self.execute_basic_block(func, *target, env)
            }
            Terminator::Return => {
                // Return the value in the return local (_0)
                let return_value = env.locals.get(&LocalId(0))
                    .cloned()
                    .unwrap_or(Value::Unit);
                Ok(return_value)
            }
            Terminator::Unreachable => {
                Err("Unreachable code executed".to_string())
            }
            _ => {
                Err(format!("Unsupported terminator: {:?}", terminator))
            }
        }
    }

    /// Evaluate an operand
    fn evaluate_operand(
        &self,
        operand: &'src Operand<'src>,
        func: &'src MirFunction<'src>,
        env: &Environment<'src>,
    ) -> Result<Value, String> {
        match operand {
            Operand::Constant(lit) => Ok(Value::from(lit.clone())),
            Operand::Copy(place) => {
                env.locals.get(&place.local)
                    .cloned()
                    .ok_or_else(|| format!("Local variable {:?} not found", place.local))
            }
        }
    }

    /// Evaluate an rvalue
    fn evaluate_rvalue(
        &self,
        rvalue: &'src Rvalue<'src>,
        func: &'src MirFunction<'src>,
        env: &Environment<'src>,
    ) -> Result<Value, String> {
        match rvalue {
            Rvalue::Use(operand) => self.evaluate_operand(operand, func, env),
            Rvalue::BinaryOp(op, lhs, rhs) => {
                let lhs_val = self.evaluate_operand(lhs, func, env)?;
                let rhs_val = self.evaluate_operand(rhs, func, env)?;

                match (lhs_val, rhs_val) {
                    (Value::I64(a), Value::I64(b)) => {
                        let result = match op {
                            BinaryOp::Add => a + b,
                            BinaryOp::Sub => a - b,
                            BinaryOp::Mul => a * b,
                            BinaryOp::Div => a / b,
                            BinaryOp::Eq => if a == b { 1 } else { 0 },
                            BinaryOp::Ne => if a != b { 1 } else { 0 },
                            BinaryOp::Lt => if a < b { 1 } else { 0 },
                            BinaryOp::Gt => if a > b { 1 } else { 0 },
                        };
                        Ok(Value::I64(result))
                    }
                    (Value::String(a), Value::String(b)) => {
                        match op {
                            BinaryOp::Add => Ok(Value::String(format!("{}{}", a, b))),
                            _ => Err("Unsupported string operation".to_string()),
                        }
                    }
                    _ => Err("Type mismatch in binary operation".to_string()),
                }
            }
            _ => Err(format!("Unsupported rvalue: {:?}", rvalue)),
        }
    }
} 