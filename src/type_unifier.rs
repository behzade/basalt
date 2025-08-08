//! type_unifier.rs
//!
//! A minimal type unifier for the MVP type checker.
//! Focuses on numeric promotion/unification and simple assignability checks
//! used by the current tests.

use crate::hir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifyError {
    IncompatibleTypes(hir::Ty, hir::Ty),
}

pub struct TypeUnifier;

impl TypeUnifier {
    pub fn is_numeric(ty: &hir::Ty) -> bool {
        matches!(
            ty,
            hir::Ty::Primitive(hir::PrimitiveTy::I32)
                | hir::Ty::Primitive(hir::PrimitiveTy::I64)
                | hir::Ty::Primitive(hir::PrimitiveTy::F64)
        )
    }

    /// Returns a common numeric type according to widening rules:
    /// i32 < i64 < f64, and any mix with f64 becomes f64.
    pub fn unify_numeric(a: &hir::Ty, b: &hir::Ty) -> Option<hir::Ty> {
        use hir::PrimitiveTy::*;
        use hir::Ty::*;
        match (a, b) {
            (Primitive(I32), Primitive(I32)) => Some(Primitive(I32)),
            (Primitive(I64), Primitive(I64)) => Some(Primitive(I64)),
            (Primitive(F64), Primitive(F64)) => Some(Primitive(F64)),

            (Primitive(I32), Primitive(I64)) | (Primitive(I64), Primitive(I32)) => {
                Some(Primitive(I64))
            }
            (Primitive(I32), Primitive(F64))
            | (Primitive(F64), Primitive(I32))
            | (Primitive(I64), Primitive(F64))
            | (Primitive(F64), Primitive(I64)) => Some(Primitive(F64)),
            _ => None,
        }
    }

    /// Returns true if a value of type `from` can be assigned to a variable of type `to`.
    /// Currently supports:
    /// - exact equality
    /// - numeric widening (i32 -> i64, i32/i64 -> f64)
    /// - identical nominal ADTs (same struct/enum path)
    pub fn is_assignable(from: &hir::Ty, to: &hir::Ty) -> bool {
        if from == to {
            return true;
        }
        if Self::is_numeric(from) && Self::is_numeric(to) {
            if let Some(common) = Self::unify_numeric(from, to) {
                return &common == to;
            }
        }
        match (from, to) {
            (hir::Ty::Adt(hir::AdtTy::Struct { name: a, .. }), hir::Ty::Adt(hir::AdtTy::Struct { name: b, .. })) => a == b,
            (hir::Ty::Adt(hir::AdtTy::Enum { name: a, .. }), hir::Ty::Adt(hir::AdtTy::Enum { name: b, .. })) => a == b,
            _ => false,
        }
    }
}


