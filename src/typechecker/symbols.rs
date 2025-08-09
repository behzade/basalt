use crate::hir;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) enum Symbol {
    Variable { ty: hir::Ty, is_mut: bool, initialized: bool },
    Function { signature: hir::HirFunctionSignature, is_public: bool, defined_in: PathBuf },
    Type { canonical_path: hir::OwnedPath, ty: hir::Ty },
}

impl super::checker::Typechecker {
    pub(crate) fn add_symbol_to_current_scope(&mut self, name: String, symbol: Symbol) {
        self.scopes.last_mut().unwrap().insert(name, symbol);
    }
}

impl super::checker::Typechecker {
    pub(crate) fn lookup_symbol(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) { return Some(symbol); }
        }
        None
    }
}


