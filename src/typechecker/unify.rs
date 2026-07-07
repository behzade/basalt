use crate::ast::BinaryOp;
use crate::hir;
use crate::typechecker::checker::Typechecker;

impl Typechecker {
    pub(crate) fn is_numeric_literal(&self, lit: &crate::ast_owned::OwnedLiteral) -> bool {
        matches!(
            lit,
            crate::ast_owned::OwnedLiteral::I8(_)
                | crate::ast_owned::OwnedLiteral::I16(_)
                | crate::ast_owned::OwnedLiteral::I32(_)
                | crate::ast_owned::OwnedLiteral::I64(_)
                | crate::ast_owned::OwnedLiteral::U8(_)
                | crate::ast_owned::OwnedLiteral::U16(_)
                | crate::ast_owned::OwnedLiteral::U32(_)
                | crate::ast_owned::OwnedLiteral::U64(_)
                | crate::ast_owned::OwnedLiteral::F32(_)
                | crate::ast_owned::OwnedLiteral::F64(_)
        )
    }

    pub(crate) fn is_arithmetic_op(&self, op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
        )
    }

    pub(crate) fn coerce_numeric_literal(
        &self,
        lit: &crate::ast_owned::OwnedLiteral,
        expected: hir::PrimitiveTy,
    ) -> Option<(hir::PrimitiveTy, String)> {
        use hir::PrimitiveTy::*;
        match expected {
            I8 | I16 | I32 | I64 => {
                let value = Self::literal_as_i128(lit)?;
                let (min, max) = Self::signed_bounds(&expected)?;
                if value < min || value > max {
                    return None;
                }
                Some((expected, value.to_string()))
            }
            Byte | U8 | U16 | U32 | U64 => {
                let value = Self::literal_as_u128(lit)?;
                let max = Self::unsigned_max(&expected)?;
                if value > max {
                    return None;
                }
                Some((expected, value.to_string()))
            }
            F32 => Some((F32, Self::literal_as_f64(lit)?.to_string())),
            F64 => Some((F64, Self::literal_as_f64(lit)?.to_string())),
            _ => None,
        }
    }

    fn literal_as_i128(lit: &crate::ast_owned::OwnedLiteral) -> Option<i128> {
        use crate::ast_owned::OwnedLiteral::*;
        Some(match lit {
            I8(v) => *v as i128,
            I16(v) => *v as i128,
            I32(v) => *v as i128,
            I64(v) => *v as i128,
            U8(v) => *v as i128,
            U16(v) => *v as i128,
            U32(v) => *v as i128,
            U64(v) => i128::try_from(*v).ok()?,
            _ => return None,
        })
    }

    fn literal_as_u128(lit: &crate::ast_owned::OwnedLiteral) -> Option<u128> {
        use crate::ast_owned::OwnedLiteral::*;
        Some(match lit {
            I8(v) => u128::try_from(*v).ok()?,
            I16(v) => u128::try_from(*v).ok()?,
            I32(v) => u128::try_from(*v).ok()?,
            I64(v) => u128::try_from(*v).ok()?,
            U8(v) => *v as u128,
            U16(v) => *v as u128,
            U32(v) => *v as u128,
            U64(v) => *v as u128,
            _ => return None,
        })
    }

    fn literal_as_f64(lit: &crate::ast_owned::OwnedLiteral) -> Option<f64> {
        use crate::ast_owned::OwnedLiteral::*;
        Some(match lit {
            I8(v) => *v as f64,
            I16(v) => *v as f64,
            I32(v) => *v as f64,
            I64(v) => *v as f64,
            U8(v) => *v as f64,
            U16(v) => *v as f64,
            U32(v) => *v as f64,
            U64(v) => *v as f64,
            F32(v) => *v as f64,
            F64(v) => *v,
            _ => return None,
        })
    }

    fn signed_bounds(ty: &hir::PrimitiveTy) -> Option<(i128, i128)> {
        use hir::PrimitiveTy::*;
        Some(match ty {
            I8 => (i8::MIN as i128, i8::MAX as i128),
            I16 => (i16::MIN as i128, i16::MAX as i128),
            I32 => (i32::MIN as i128, i32::MAX as i128),
            I64 => (i64::MIN as i128, i64::MAX as i128),
            _ => return None,
        })
    }

    fn unsigned_max(ty: &hir::PrimitiveTy) -> Option<u128> {
        use hir::PrimitiveTy::*;
        Some(match ty {
            Byte | U8 => u8::MAX as u128,
            U16 => u16::MAX as u128,
            U32 => u32::MAX as u128,
            U64 => u64::MAX as u128,
            _ => return None,
        })
    }
}
