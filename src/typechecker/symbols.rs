use crate::hir;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) enum Symbol {
    Variable {
        ty: hir::Ty,
        is_mut: bool,
        initialized: bool,
        decl_span: Option<crate::token::SimpleSpan>,
    },
    Function {
        signature: hir::HirFunctionSignature,
        is_public: bool,
        defined_in: PathBuf,
        decl_span: Option<crate::token::SimpleSpan>,
    },
    Type {
        canonical_path: hir::OwnedPath,
        ty: hir::Ty,
    },
}

impl super::checker::Typechecker {
    pub(crate) fn add_symbol_to_current_scope(&mut self, name: String, symbol: Symbol) {
        // Prevent redeclaration in the same (innermost) scope
        if let Some(current) = self.scopes.last_mut() {
            if current.contains_key(&name) {
                // Push a type error if we have context available elsewhere. Since we do not
                // have span/path here, leave enforcement to callers that can emit an error.
                // Silently skip insertion to avoid poisoning scope with duplicate.
                return;
            }
            current.insert(name, symbol);
        }
    }
}

impl super::checker::Typechecker {
    pub(crate) fn lookup_symbol(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }
}
