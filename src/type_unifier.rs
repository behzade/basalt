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

#[cfg(test)]
mod tests {
    use super::*;

    fn i32_ty() -> hir::Ty {
        hir::Ty::Primitive(hir::PrimitiveTy::I32)
    }

    fn str_ty() -> hir::Ty {
        hir::Ty::Primitive(hir::PrimitiveTy::Str)
    }

    fn option_ty(arg: hir::Ty) -> hir::Ty {
        hir::Ty::Adt(hir::AdtTy::Enum {
            name: vec!["Option".to_string()],
            generics: vec![arg],
        })
    }

    #[test]
    fn generic_adt_assignability_is_invariant() {
        assert!(TypeUnifier::is_assignable(
            &option_ty(i32_ty()),
            &option_ty(i32_ty())
        ));
        assert!(!TypeUnifier::is_assignable(
            &option_ty(i32_ty()),
            &option_ty(str_ty())
        ));
    }
}

pub struct TypeUnifier;

impl TypeUnifier {
    pub fn is_numeric(ty: &hir::Ty) -> bool {
        matches!(ty, hir::Ty::Primitive(prim) if Self::numeric_width(prim).is_some())
    }

    /// Returns a common numeric type according to widening rules. Signed and
    /// unsigned integers widen independently; mixing signed and unsigned uses
    /// the next signed width that can represent both when available. Any mix
    /// with a float becomes f64 unless all operands can be represented by f32.
    pub fn unify_numeric(a: &hir::Ty, b: &hir::Ty) -> Option<hir::Ty> {
        use hir::PrimitiveTy::*;
        use hir::Ty::*;
        match (a, b) {
            (Primitive(a), Primitive(b)) => {
                if a == b {
                    return Some(Primitive(a.clone()));
                }

                if Self::is_float(a) || Self::is_float(b) {
                    return Some(Primitive(if Self::fits_in_f32(a) && Self::fits_in_f32(b) {
                        F32
                    } else {
                        F64
                    }));
                }

                let a_signed = Self::is_signed_integer(a)?;
                let b_signed = Self::is_signed_integer(b)?;
                let width = Self::numeric_width(a)?.max(Self::numeric_width(b)?);

                Some(Primitive(match (a_signed, b_signed) {
                    (true, true) => Self::signed_integer_for_width(width)?,
                    (false, false) => Self::unsigned_integer_for_width(width),
                    (true, false) | (false, true) => Self::signed_integer_for_width(width * 2)?,
                }))
            }
            _ => None,
        }
    }

    fn numeric_width(ty: &hir::PrimitiveTy) -> Option<u16> {
        use hir::PrimitiveTy::*;
        Some(match ty {
            Byte | I8 | U8 => 8,
            I16 | U16 => 16,
            I32 | U32 | F32 => 32,
            I64 | U64 | F64 => 64,
            Bool | Str => return None,
        })
    }

    fn is_float(ty: &hir::PrimitiveTy) -> bool {
        matches!(ty, hir::PrimitiveTy::F32 | hir::PrimitiveTy::F64)
    }

    fn fits_in_f32(ty: &hir::PrimitiveTy) -> bool {
        use hir::PrimitiveTy::*;
        matches!(ty, Byte | I8 | I16 | U8 | U16 | F32)
    }

    fn is_signed_integer(ty: &hir::PrimitiveTy) -> Option<bool> {
        use hir::PrimitiveTy::*;
        match ty {
            I8 | I16 | I32 | I64 => Some(true),
            Byte | U8 | U16 | U32 | U64 => Some(false),
            Bool | F32 | F64 | Str => None,
        }
    }

    fn signed_integer_for_width(width: u16) -> Option<hir::PrimitiveTy> {
        use hir::PrimitiveTy::*;
        Some(match width {
            0..=8 => I8,
            9..=16 => I16,
            17..=32 => I32,
            33..=64 => I64,
            _ => return None,
        })
    }

    fn unsigned_integer_for_width(width: u16) -> hir::PrimitiveTy {
        use hir::PrimitiveTy::*;
        match width {
            0..=8 => U8,
            9..=16 => U16,
            17..=32 => U32,
            _ => U64,
        }
    }

    pub fn is_lossless_numeric_conversion(from: &hir::PrimitiveTy, to: &hir::PrimitiveTy) -> bool {
        use hir::PrimitiveTy::*;
        if from == to {
            return true;
        }

        match (from, to) {
            (Byte, U8) | (U8, Byte) => true,
            (F32, F64) => true,
            (F64, F32) => false,
            (from, F32) => Self::fits_in_f32(from),
            (from, F64) => matches!(from, Byte | I8 | I16 | I32 | U8 | U16 | U32 | F32),
            (F32 | F64, _) => false,
            _ => {
                let Some((from_min, from_max)) = Self::integer_bounds(from) else {
                    return false;
                };
                let Some((to_min, to_max)) = Self::integer_bounds(to) else {
                    return false;
                };
                from_min >= to_min && from_max <= to_max
            }
        }
    }

    fn integer_bounds(ty: &hir::PrimitiveTy) -> Option<(i128, i128)> {
        use hir::PrimitiveTy::*;
        Some(match ty {
            Byte | U8 => (0, u8::MAX as i128),
            I8 => (i8::MIN as i128, i8::MAX as i128),
            I16 => (i16::MIN as i128, i16::MAX as i128),
            I32 => (i32::MIN as i128, i32::MAX as i128),
            I64 => (i64::MIN as i128, i64::MAX as i128),
            U16 => (0, u16::MAX as i128),
            U32 => (0, u32::MAX as i128),
            U64 => (0, u64::MAX as i128),
            Bool | F32 | F64 | Str => return None,
        })
    }

    /// Returns true if a value of type `from` can be assigned to a variable of type `to`.
    /// Currently supports:
    /// - exact equality
    /// - never (`!`) as bottom, assignable to any type
    /// - numeric widening (i32 -> i64, i32/i64 -> f64)
    /// - identical nominal ADTs (same struct/enum path and generic arguments)
    pub fn is_assignable(from: &hir::Ty, to: &hir::Ty) -> bool {
        if from == to {
            return true;
        }
        if crate::typechecker::checker::Typechecker::is_memory_address_ty(from)
            && crate::typechecker::checker::Typechecker::is_memory_address_ty(to)
        {
            return true;
        }
        if matches!(from, hir::Ty::Special(hir::SpecialTy::Never)) {
            return true;
        }
        if let (hir::Ty::Primitive(from), hir::Ty::Primitive(to)) = (from, to) {
            return Self::is_lossless_numeric_conversion(from, to);
        }
        match (from, to) {
            (
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: a,
                    generics: a_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: b,
                    generics: b_generics,
                }),
            ) => {
                let same_memory_address = a.last().map(String::as_str) == Some("MemoryAddress")
                    && b.last().map(String::as_str) == Some("MemoryAddress");
                (a == b || same_memory_address) && Self::same_generic_args(a_generics, b_generics)
            }
            (
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: a,
                    generics: a_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: b,
                    generics: b_generics,
                }),
            ) => a == b && Self::same_generic_args(a_generics, b_generics),
            (hir::Ty::Handler { effects: a }, hir::Ty::Handler { effects: b }) => a == b,

            // Allow assigning a specific enum variant (struct-like `Enum::Variant`) to its parent enum type
            (
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: variant_path,
                    generics: variant_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: enum_path,
                    generics: enum_generics,
                }),
            ) => {
                if variant_path.len() == enum_path.len() + 1 {
                    // Check prefix equality: `Enum::Variant` starts with `Enum`
                    variant_path
                        .iter()
                        .zip(enum_path.iter())
                        .all(|(a, b)| a == b)
                        && Self::same_generic_args(variant_generics, enum_generics)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn same_generic_args(a: &[hir::Ty], b: &[hir::Ty]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(a, b)| Self::same_type_invariant(a, b))
    }

    fn same_type_invariant(a: &hir::Ty, b: &hir::Ty) -> bool {
        if a == b {
            return true;
        }
        match (a, b) {
            (hir::Ty::Array(a), hir::Ty::Array(b)) => Self::same_type_invariant(a, b),
            (
                hir::Ty::Map {
                    key: a_key,
                    value: a_value,
                },
                hir::Ty::Map {
                    key: b_key,
                    value: b_value,
                },
            ) => a_key == b_key && Self::same_type_invariant(a_value, b_value),
            (
                hir::Ty::Function {
                    param_types: a_params,
                    ret_type: a_ret,
                    effects: a_effects,
                },
                hir::Ty::Function {
                    param_types: b_params,
                    ret_type: b_ret,
                    effects: b_effects,
                },
            ) => {
                Self::same_generic_args(a_params, b_params)
                    && Self::same_type_invariant(a_ret, b_ret)
                    && Self::same_generic_args(a_effects, b_effects)
            }
            (
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: a_name,
                    generics: a_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: b_name,
                    generics: b_generics,
                }),
            )
            | (
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: a_name,
                    generics: a_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: b_name,
                    generics: b_generics,
                }),
            )
            | (
                hir::Ty::Adt(hir::AdtTy::Effect {
                    name: a_name,
                    generics: a_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Effect {
                    name: b_name,
                    generics: b_generics,
                }),
            ) => a_name == b_name && Self::same_generic_args(a_generics, b_generics),
            (hir::Ty::Handler { effects: a }, hir::Ty::Handler { effects: b }) => {
                Self::same_generic_args(a, b)
            }
            _ => false,
        }
    }
}
