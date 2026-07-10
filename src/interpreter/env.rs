use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::value::Value;

#[derive(Debug)]
pub struct RuntimeError(pub String);

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Default, Debug, Clone)]
pub struct Env {
    pub(crate) scopes: Vec<Scope>, // lexical scopes, last is current
}

pub(crate) type Scope = HashMap<String, Binding>;

#[derive(Debug, Clone)]
pub(crate) struct Binding {
    value: Rc<RefCell<Value>>,
    mutable: bool,
}

impl Binding {
    pub(crate) fn stack_size_bytes(&self) -> usize {
        self.value.borrow().stack_size_bytes()
    }

    pub(crate) fn value(&self) -> Value {
        self.value.borrow().clone()
    }
}

impl Env {
    pub fn new() -> Self {
        Env {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String, value: Value, mutable: bool) {
        if let Some(current) = self.scopes.last_mut() {
            current.insert(
                name,
                Binding {
                    value: Rc::new(RefCell::new(value)),
                    mutable,
                },
            );
        }
    }

    pub(crate) fn define_alias(&mut self, name: String, binding: Binding) {
        if let Some(current) = self.scopes.last_mut() {
            current.insert(name, binding);
        }
    }

    pub(crate) fn binding(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<()> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                if !binding.mutable {
                    return Err(RuntimeError(format!(
                        "Cannot assign to immutable variable: {}",
                        name
                    )));
                }
                *binding.value.borrow_mut() = value;
                return Ok(());
            }
        }
        Err(RuntimeError(format!("Undefined variable: {}", name)))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.value.borrow().clone());
            }
        }
        None
    }
}
