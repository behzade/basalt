//! typechecker/unification.rs
//!
//! Contains the logic for type unification.

use super::{TypeChecker, TypeError};
use crate::hir::Ty;

impl<'src> TypeChecker<'src> {
    /// Unifies two types, returning an error if they cannot be unified.
    pub fn unify(&mut self, ty1: &Ty<'src>, ty2: &Ty<'src>) -> Result<(), TypeError<'src>> {
        match (ty1, ty2) {
            // Same types unify
            | (Ty::Bool, Ty::Bool)
            | (Ty::I32, Ty::I32)
            | (Ty::I64, Ty::I64)
            | (Ty::F64, Ty::F64)
            | (Ty::Str, Ty::Str)
            | (Ty::Unit, Ty::Unit) => Ok(()),

            // Integer promotion: i32 can be promoted to i64
            | (Ty::I32, Ty::I64) | (Ty::I64, Ty::I32) => Ok(()),

            // Inference variables unify with anything
            (Ty::Infer(id), other) | (other, Ty::Infer(id)) => {
                self.substitutions.insert(*id, other.clone());
                Ok(())
            }

            // Arrays unify if their element types unify
            (Ty::Array(elem1), Ty::Array(elem2)) => self.unify(elem1, elem2),

            // Maps unify if their key and value types unify
            (Ty::Map { key: key1, value: value1 }, Ty::Map { key: key2, value: value2 }) => {
                self.unify(key1, key2)?;
                self.unify(value1, value2)
            }

            // ADTs unify if they have the same name and their generics unify
            (Ty::Adt { name: name1, generics: generics1 }, Ty::Adt { name: name2, generics: generics2 }) => {
                if name1 != name2 || generics1.len() != generics2.len() {
                    return Err(TypeError::MismatchedTypes {
                        expected: ty1.clone(),
                        found: ty2.clone(),
                    });
                }
                for (g1, g2) in generics1.iter().zip(generics2.iter()) {
                    self.unify(g1, g2)?;
                }
                Ok(())
            }

            // Functions unify if their parameter and return types unify
            (Ty::Function { param_types: params1, ret_type: ret1 }, Ty::Function { param_types: params2, ret_type: ret2 }) => {
                if params1.len() != params2.len() {
                    return Err(TypeError::MismatchedTypes {
                        expected: ty1.clone(),
                        found: ty2.clone(),
                    });
                }
                for (p1, p2) in params1.iter().zip(params2.iter()) {
                    self.unify(p1, p2)?;
                }
                self.unify(ret1, ret2)
            }

            // Error types unify with anything
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),

            // Otherwise, types don't unify
            _ => Err(TypeError::MismatchedTypes {
                expected: ty1.clone(),
                found: ty2.clone(),
            }),
        }
    }

    fn unify_variable(&mut self, id: u32, ty: &Ty<'src>) -> Result<(), TypeError<'src>> {
        if let Ty::Infer(other_id) = ty {
            if *other_id == id {
                return Ok(());
            }
        }

        if self.occurs(id, ty) {
            return Err(TypeError::UnificationError(Ty::Infer(id), ty.clone()));
        }

        self.substitutions.insert(id, ty.clone());
        Ok(())
    }

    fn occurs(&self, id: u32, ty: &Ty<'src>) -> bool {
        match self.resolve_type(ty) {
            Ty::Infer(other_id) => id == other_id,
            // FIX: Dereference the Box<Ty> to get a &Ty for the recursive call.
            Ty::Array(inner) => self.occurs(id, &inner),
            Ty::Map { key, value } => self.occurs(id, &key) || self.occurs(id, &value),
            Ty::Adt { generics, .. } => generics.iter().any(|g| self.occurs(id, g)),
            Ty::Function {
                param_types,
                ret_type,
            } => param_types.iter().any(|p| self.occurs(id, p)) || self.occurs(id, &ret_type),
            _ => false,
        }
    }

    pub fn resolve_type(&self, ty: &Ty<'src>) -> Ty<'src> {
        match ty {
            Ty::Infer(id) => {
                if let Some(subst_ty) = self.substitutions.get(id) {
                    self.resolve_type(subst_ty)
                } else {
                    ty.clone()
                }
            }
            Ty::Array(inner) => Ty::Array(Box::new(self.resolve_type(inner))),
            Ty::Map { key, value } => Ty::Map {
                key: Box::new(self.resolve_type(key)),
                value: Box::new(self.resolve_type(value)),
            },
            Ty::Adt { name, generics } => Ty::Adt {
                name: name.clone(),
                generics: generics.iter().map(|g| self.resolve_type(g)).collect(),
            },
            _ => ty.clone(),
        }
    }
}
