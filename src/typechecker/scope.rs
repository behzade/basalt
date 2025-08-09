use std::collections::HashMap;

impl super::checker::Typechecker {
    pub(crate) fn enter_scope(&mut self) { self.scopes.push(HashMap::new()); }
    pub(crate) fn leave_scope(&mut self) { self.scopes.pop(); }
}


