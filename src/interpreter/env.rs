use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::value::{AllocationOwner, Value};

#[derive(Debug)]
pub struct RuntimeError(pub String);

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Default, Debug, Clone)]
pub struct Env {
    pub(crate) scopes: Vec<Scope>, // lexical scopes, last is current
    region_scopes: Vec<Vec<AllocationOwner>>,
}

pub(crate) type Scope = HashMap<String, Binding>;

#[derive(Debug, Clone)]
pub(crate) struct Binding {
    value: Rc<RefCell<Value>>,
    mutable: bool,
    destination: Option<AllocationOwner>,
}

impl Binding {
    pub(crate) fn stack_size_bytes(&self) -> usize {
        self.value.borrow().stack_size_bytes()
    }

    pub(crate) fn value(&self) -> Value {
        self.value.borrow().clone()
    }

    pub(crate) fn destination(&self) -> Option<AllocationOwner> {
        self.destination
    }

    pub(crate) fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub(crate) fn same_storage(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.value, &other.value)
    }

    pub(crate) fn detached_with(&self, value: Value, destination: AllocationOwner) -> Self {
        Self {
            value: Rc::new(RefCell::new(value)),
            mutable: self.mutable,
            destination: Some(destination),
        }
    }

    pub(crate) fn replace(&self, value: Value) {
        *self.value.borrow_mut() = value;
    }
}

impl Env {
    pub fn new() -> Self {
        Env {
            scopes: vec![HashMap::new()],
            region_scopes: vec![vec![]],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.region_scopes.push(vec![]);
    }

    pub fn pop_scope(&mut self) -> Vec<AllocationOwner> {
        self.scopes.pop();
        self.region_scopes.pop().unwrap_or_default()
    }

    pub(crate) fn register_region(&mut self, owner: AllocationOwner) {
        if let Some(regions) = self.region_scopes.last_mut() {
            regions.push(owner);
        }
    }

    pub(crate) fn with_captured(scopes: Vec<Scope>) -> Self {
        let region_scopes = vec![vec![]; scopes.len()];
        Self {
            scopes,
            region_scopes,
        }
    }

    pub fn define(
        &mut self,
        name: String,
        value: Value,
        mutable: bool,
        destination: Option<AllocationOwner>,
    ) {
        if let Some(current) = self.scopes.last_mut() {
            current.insert(
                name,
                Binding {
                    value: Rc::new(RefCell::new(value)),
                    mutable,
                    destination,
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

    pub(crate) fn capture(&self, names: &HashSet<String>) -> Vec<Scope> {
        let captured = names
            .iter()
            .filter_map(|name| self.binding(name).map(|binding| (name.clone(), binding)))
            .collect::<Scope>();
        if captured.is_empty() {
            vec![]
        } else {
            vec![captured]
        }
    }
}
