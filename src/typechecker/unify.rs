use crate::ast::BinaryOp;
use crate::hir;
use crate::typechecker::checker::Typechecker;

impl Typechecker {
    pub(crate) fn is_numeric_type(&self, ty: &hir::Ty) -> bool {
        matches!(
            ty,
            hir::Ty::Primitive(hir::PrimitiveTy::I32)
                | hir::Ty::Primitive(hir::PrimitiveTy::I64)
                | hir::Ty::Primitive(hir::PrimitiveTy::F64)
        )
    }

    pub(crate) fn is_numeric_literal(&self, lit: &crate::ast_owned::OwnedLiteral) -> bool {
        matches!(
            lit,
            crate::ast_owned::OwnedLiteral::I32(_)
                | crate::ast_owned::OwnedLiteral::I64(_)
                | crate::ast_owned::OwnedLiteral::F64(_)
        )
    }

    pub(crate) fn is_arithmetic_op(&self, op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
        )
    }

    pub(crate) fn unify_numeric_types(&self, a: hir::Ty, b: hir::Ty) -> Option<hir::Ty> {
        use hir::PrimitiveTy::*;
        use hir::Ty::*;
        match (a, b) {
            (Primitive(I64), Primitive(I32)) | (Primitive(I32), Primitive(I64)) => {
                Some(Primitive(I64))
            }
            (Primitive(F64), Primitive(I32))
            | (Primitive(I32), Primitive(F64))
            | (Primitive(F64), Primitive(I64))
            | (Primitive(I64), Primitive(F64)) => Some(Primitive(F64)),
            (Primitive(I32), Primitive(I32)) => Some(Primitive(I32)),
            (Primitive(I64), Primitive(I64)) => Some(Primitive(I64)),
            (Primitive(F64), Primitive(F64)) => Some(Primitive(F64)),
            (x, y) if x == y => Some(x),
            _ => None,
        }
    }

    pub(crate) fn coerce_numeric_literal(
        &self,
        lit: &crate::ast_owned::OwnedLiteral,
        expected: hir::PrimitiveTy,
    ) -> Option<(hir::PrimitiveTy, String)> {
        use hir::PrimitiveTy::*;
        match (lit, expected) {
            (crate::ast_owned::OwnedLiteral::I32(v), I32) => Some((I32, v.to_string())),
            (crate::ast_owned::OwnedLiteral::I64(v), I64) => Some((I64, v.to_string())),
            (crate::ast_owned::OwnedLiteral::F64(v), F64) => Some((F64, v.to_string())),
            (crate::ast_owned::OwnedLiteral::I64(v), I32) => Some((I32, v.to_string())),
            (crate::ast_owned::OwnedLiteral::I32(v), I64) => Some((I64, v.to_string())),
            (crate::ast_owned::OwnedLiteral::I32(v), F64) => Some((F64, (*v as f64).to_string())),
            (crate::ast_owned::OwnedLiteral::I64(v), F64) => Some((F64, (*v as f64).to_string())),
            (crate::ast_owned::OwnedLiteral::F64(v), I32) => Some((I32, (*v as i32).to_string())),
            (crate::ast_owned::OwnedLiteral::F64(v), I64) => Some((I64, (*v as i64).to_string())),
            _ => None,
        }
    }
}
