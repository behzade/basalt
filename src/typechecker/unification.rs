//! typechecker/unification.rs
//!
//! Contains the logic for type unification.

use super::{TypeError, TypeChecker};
use crate::hir::Ty;

impl<'src> TypeChecker<'src> {
    pub fn unify(&mut self, ty1: &Ty<'src>, ty2: &Ty<'src>) -> Result<(), TypeError<'src>> {
        let resolved_ty1 = self.resolve_type(ty1);
        let resolved_ty2 = self.resolve_type(ty2);

        match (resolved_ty1, resolved_ty2) {
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),
            (Ty::Infer(id), ty) | (ty, Ty::Infer(id)) => self.unify_variable(id, &ty),
            (Ty::Bool, Ty::Bool)
            | (Ty::I64, Ty::I64)
            | (Ty::F64, Ty::F64)
            | (Ty::Str, Ty::Str)
            | (Ty::Unit, Ty::Unit) => Ok(()),
            
            // FIX: Dereference the Box<Ty> to get a &Ty for the recursive call.
            (Ty::Array(inner1), Ty::Array(inner2)) => self.unify(&inner1, &inner2),

            (
                Ty::Map {
                    key: key1,
                    value: value1,
                },
                Ty::Map {
                    key: key2,
                    value: value2,
                },
            ) => {
                // FIX: Dereference the Box<Ty> to get a &Ty.
                self.unify(&key1, &key2)?;
                self.unify(&value1, &value2)
            }

            (
                Ty::Adt {
                    name: name1,
                    generics: generics1,
                },
                Ty::Adt {
                    name: name2,
                    generics: generics2,
                },
            ) => {
                if name1 == name2 && generics1.len() == generics2.len() {
                    for (g1, g2) in generics1.iter().zip(generics2.iter()) {
                        self.unify(g1, g2)?;
                    }
                    Ok(())
                } else {
                    Err(TypeError::UnificationError(ty1.clone(), ty2.clone()))
                }
            }
            (t1, t2) => {
                if t1 == t2 {
                    Ok(())
                } else {
                    Err(TypeError::MismatchedTypes {
                        expected: t1.clone(),
                        found: t2.clone(),
                    })
                }
            }
        }
    }

    fn unify_variable(&mut self, id: u32, ty: &Ty<'src>) -> Result<(), TypeError<'src>> {
        if let Ty::Infer(other_id) = ty {
            if *other_id == id {
                return Ok(());
            }
        }

        if self.occurs(id, ty) {
            return Err(TypeError::UnificationError(
                Ty::Infer(id),
                ty.clone(),
            ));
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
            } => {
                param_types.iter().any(|p| self.occurs(id, p)) || self.occurs(id, &ret_type)
            }
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
