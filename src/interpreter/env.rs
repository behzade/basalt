use std::collections::HashMap;

use super::value::Value;

#[derive(Debug)]
pub struct RuntimeError(pub String);

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Default, Debug, Clone)]
pub struct Env {
    pub(crate) scopes: Vec<HashMap<String, Value>>, // lexical scopes, last is current
}

impl Env {
    pub fn new() -> Self {
        Env { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String, value: Value) {
        if let Some(current) = self.scopes.last_mut() {
            current.insert(name, value);
        }
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(RuntimeError(format!("Undefined variable: {}", name)))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}


