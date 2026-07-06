use crate::hir;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::symbols::Symbol;

impl Typechecker {
    pub(crate) fn mark_variable_initialized(&mut self, name: &str) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(Symbol::Variable { initialized, .. }) = scope.get_mut(name) {
                *initialized = true;
                break;
            }
        }
    }

    pub(crate) fn lower_literal(
        &self,
        lit: crate::ast_owned::OwnedLiteral,
    ) -> (hir::PrimitiveTy, String) {
        match lit {
            crate::ast_owned::OwnedLiteral::Bool(b) => (hir::PrimitiveTy::Bool, b.to_string()),
            crate::ast_owned::OwnedLiteral::I8(i) => (hir::PrimitiveTy::I8, i.to_string()),
            crate::ast_owned::OwnedLiteral::I16(i) => (hir::PrimitiveTy::I16, i.to_string()),
            crate::ast_owned::OwnedLiteral::I32(i) => (hir::PrimitiveTy::I32, i.to_string()),
            crate::ast_owned::OwnedLiteral::I64(i) => (hir::PrimitiveTy::I64, i.to_string()),
            crate::ast_owned::OwnedLiteral::U8(i) => (hir::PrimitiveTy::U8, i.to_string()),
            crate::ast_owned::OwnedLiteral::U16(i) => (hir::PrimitiveTy::U16, i.to_string()),
            crate::ast_owned::OwnedLiteral::U32(i) => (hir::PrimitiveTy::U32, i.to_string()),
            crate::ast_owned::OwnedLiteral::U64(i) => (hir::PrimitiveTy::U64, i.to_string()),
            crate::ast_owned::OwnedLiteral::F32(f) => (hir::PrimitiveTy::F32, f.to_string()),
            crate::ast_owned::OwnedLiteral::F64(f) => (hir::PrimitiveTy::F64, f.to_string()),
            crate::ast_owned::OwnedLiteral::Str(s) => (hir::PrimitiveTy::Str, s),
            crate::ast_owned::OwnedLiteral::Unit => (hir::PrimitiveTy::Bool, "false".to_string()),
        }
    }

    pub(crate) fn lower_binary_op(&self, op: crate::ast::BinaryOp) -> hir::BinaryOp {
        use crate::ast::BinaryOp as A;
        match op {
            A::Add => hir::BinaryOp::Add,
            A::Sub => hir::BinaryOp::Sub,
            A::Mul => hir::BinaryOp::Mul,
            A::Div => hir::BinaryOp::Div,
            A::Mod => hir::BinaryOp::Mod,
            A::Assign => hir::BinaryOp::Assign,
            A::Eq => hir::BinaryOp::Eq,
            A::Ne => hir::BinaryOp::Ne,
            A::Lt => hir::BinaryOp::Lt,
            A::Lte => hir::BinaryOp::Lte,
            A::Gt => hir::BinaryOp::Gt,
            A::Gte => hir::BinaryOp::Gte,
            A::And => hir::BinaryOp::And,
            A::Or => hir::BinaryOp::Or,
            A::BinaryXor => hir::BinaryOp::Xor,
            A::BinaryAnd => hir::BinaryOp::And,
            A::BinaryOr => hir::BinaryOp::Or,
            A::BitShiftLeft => hir::BinaryOp::BitShiftLeft,
            A::BitShiftRight => hir::BinaryOp::BitShiftRight,
        }
    }

    pub(crate) fn lookup_struct_field_type(
        &self,
        path: &hir::OwnedPath,
        field: &str,
    ) -> Option<&hir::Ty> {
        match self.type_definitions.get(path) {
            Some(hir::Item::Struct(def)) => {
                def.fields.iter().find(|f| f.name == field).map(|f| &f.ty)
            }
            _ => None,
        }
    }

    pub(crate) fn find_union_variant(
        &self,
        variant: &str,
    ) -> Option<(hir::OwnedPath, Option<Vec<hir::Ty>>)> {
        for (union_path, variants) in &self.union_variants {
            if let Some((_, payload)) = variants.iter().find(|(name, _)| name == variant).cloned() {
                return Some((union_path.clone(), payload));
            }
        }
        None
    }

    pub(crate) fn resolve_effect_op(
        &self,
        path: &hir::OwnedPath,
    ) -> Option<(hir::Ty, Vec<hir::Ty>)> {
        if path.len() != 2 {
            return None;
        }
        let effect_name = vec![path[0].clone()];
        let op_name = &path[1];
        match self.type_definitions.get(&effect_name) {
            Some(hir::Item::Effect(def)) => {
                for sig in &def.operations {
                    if &sig.name == op_name {
                        return Some((
                            sig.ret_type.clone(),
                            sig.params.iter().map(|p| p.ty.clone()).collect(),
                        ));
                    }
                }
                None
            }
            _ => None,
        }
    }
}
