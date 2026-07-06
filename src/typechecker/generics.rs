use std::collections::HashMap;

use crate::hir;
use crate::typechecker::checker::Typechecker;

impl Typechecker {
    pub(crate) fn generic_bindings_for(
        &self,
        name: &hir::OwnedPath,
        generics: &[hir::Ty],
    ) -> HashMap<String, hir::Ty> {
        self.type_alias_generics
            .get(name)
            .into_iter()
            .flat_map(|params| params.iter().zip(generics.iter()))
            .map(|(param, arg)| (param.clone(), arg.clone()))
            .collect()
    }

    pub(crate) fn substitute_generics_in_ty(
        ty: &hir::Ty,
        bindings: &HashMap<String, hir::Ty>,
    ) -> hir::Ty {
        match ty {
            hir::Ty::Generic(name) => bindings
                .get(name)
                .cloned()
                .unwrap_or_else(|| hir::Ty::Generic(name.clone())),
            hir::Ty::Array(elem) => {
                hir::Ty::Array(Box::new(Self::substitute_generics_in_ty(elem, bindings)))
            }
            hir::Ty::Map { key, value } => hir::Ty::Map {
                key: key.clone(),
                value: Box::new(Self::substitute_generics_in_ty(value, bindings)),
            },
            hir::Ty::Function {
                param_types,
                ret_type,
                effects,
            } => hir::Ty::Function {
                param_types: param_types
                    .iter()
                    .map(|ty| Self::substitute_generics_in_ty(ty, bindings))
                    .collect(),
                ret_type: Box::new(Self::substitute_generics_in_ty(ret_type, bindings)),
                effects: effects
                    .iter()
                    .map(|ty| Self::substitute_generics_in_ty(ty, bindings))
                    .collect(),
            },
            hir::Ty::Handler { effects } => hir::Ty::Handler {
                effects: effects
                    .iter()
                    .map(|ty| Self::substitute_generics_in_ty(ty, bindings))
                    .collect(),
            },
            hir::Ty::Adt(hir::AdtTy::Struct { name, generics }) => {
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: name.clone(),
                    generics: generics
                        .iter()
                        .map(|ty| Self::substitute_generics_in_ty(ty, bindings))
                        .collect(),
                })
            }
            hir::Ty::Adt(hir::AdtTy::Enum { name, generics }) => hir::Ty::Adt(hir::AdtTy::Enum {
                name: name.clone(),
                generics: generics
                    .iter()
                    .map(|ty| Self::substitute_generics_in_ty(ty, bindings))
                    .collect(),
            }),
            hir::Ty::Adt(hir::AdtTy::Effect { name, generics }) => {
                hir::Ty::Adt(hir::AdtTy::Effect {
                    name: name.clone(),
                    generics: generics
                        .iter()
                        .map(|ty| Self::substitute_generics_in_ty(ty, bindings))
                        .collect(),
                })
            }
            other => other.clone(),
        }
    }

    pub(crate) fn contains_generic_ty(ty: &hir::Ty) -> bool {
        match ty {
            hir::Ty::Generic(_) => true,
            hir::Ty::Array(elem) => Self::contains_generic_ty(elem),
            hir::Ty::Map { value, .. } => Self::contains_generic_ty(value),
            hir::Ty::Function {
                param_types,
                ret_type,
                effects,
            } => {
                param_types.iter().any(Self::contains_generic_ty)
                    || Self::contains_generic_ty(ret_type)
                    || effects.iter().any(Self::contains_generic_ty)
            }
            hir::Ty::Handler { effects } => effects.iter().any(Self::contains_generic_ty),
            hir::Ty::Adt(hir::AdtTy::Struct { generics, .. })
            | hir::Ty::Adt(hir::AdtTy::Enum { generics, .. })
            | hir::Ty::Adt(hir::AdtTy::Effect { generics, .. }) => {
                generics.iter().any(Self::contains_generic_ty)
            }
            _ => false,
        }
    }

    pub(crate) fn infer_generic_bindings_from_ty(
        pattern: &hir::Ty,
        actual: &hir::Ty,
        bindings: &mut HashMap<String, hir::Ty>,
    ) -> Result<(), String> {
        match (pattern, actual) {
            (hir::Ty::Generic(name), actual) => {
                if let Some(existing) = bindings.get(name) {
                    if existing != actual {
                        return Err(format!(
                            "Conflicting inference for generic `{}`: {} vs {}",
                            name,
                            Typechecker::format_ty(existing),
                            Typechecker::format_ty(actual)
                        ));
                    }
                } else {
                    bindings.insert(name.clone(), actual.clone());
                }
                Ok(())
            }
            (hir::Ty::Array(p), hir::Ty::Array(a)) => {
                Self::infer_generic_bindings_from_ty(p, a, bindings)
            }
            (
                hir::Ty::Map {
                    key: p_key,
                    value: p_value,
                },
                hir::Ty::Map {
                    key: a_key,
                    value: a_value,
                },
            ) => {
                if p_key != a_key {
                    return Err(format!(
                        "Map key type mismatch during generic inference: expected {:?}, found {:?}",
                        p_key, a_key
                    ));
                }
                Self::infer_generic_bindings_from_ty(p_value, a_value, bindings)
            }
            (
                hir::Ty::Function {
                    param_types: p_params,
                    ret_type: p_ret,
                    effects: p_effects,
                },
                hir::Ty::Function {
                    param_types: a_params,
                    ret_type: a_ret,
                    effects: a_effects,
                },
            ) => {
                if p_params.len() != a_params.len() {
                    return Err(format!(
                        "Function arity mismatch during generic inference: expected {}, found {}",
                        p_params.len(),
                        a_params.len()
                    ));
                }
                if p_effects.len() != a_effects.len() {
                    return Err(format!(
                        "Function effect arity mismatch during generic inference: expected {}, found {}",
                        p_effects.len(),
                        a_effects.len()
                    ));
                }
                for (p, a) in p_params.iter().zip(a_params.iter()) {
                    Self::infer_generic_bindings_from_ty(p, a, bindings)?;
                }
                Self::infer_generic_bindings_from_ty(p_ret, a_ret, bindings)?;
                for (p, a) in p_effects.iter().zip(a_effects.iter()) {
                    Self::infer_generic_bindings_from_ty(p, a, bindings)?;
                }
                Ok(())
            }
            (hir::Ty::Handler { effects: p_effects }, hir::Ty::Handler { effects: a_effects }) => {
                if p_effects.len() != a_effects.len() {
                    return Err(format!(
                        "Handler effect arity mismatch during generic inference: expected {}, found {}",
                        p_effects.len(),
                        a_effects.len()
                    ));
                }
                for (p, a) in p_effects.iter().zip(a_effects.iter()) {
                    Self::infer_generic_bindings_from_ty(p, a, bindings)?;
                }
                Ok(())
            }
            (
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: p_name,
                    generics: p_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Struct {
                    name: a_name,
                    generics: a_generics,
                }),
            )
            | (
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: p_name,
                    generics: p_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Enum {
                    name: a_name,
                    generics: a_generics,
                }),
            )
            | (
                hir::Ty::Adt(hir::AdtTy::Effect {
                    name: p_name,
                    generics: p_generics,
                }),
                hir::Ty::Adt(hir::AdtTy::Effect {
                    name: a_name,
                    generics: a_generics,
                }),
            ) => {
                if p_name != a_name {
                    return Self::generic_shape_error(pattern, actual);
                }
                if p_generics.len() != a_generics.len() {
                    return Err(format!(
                        "Generic arity mismatch for `{}`: expected {}, found {}",
                        p_name.join("::"),
                        p_generics.len(),
                        a_generics.len()
                    ));
                }
                for (p, a) in p_generics.iter().zip(a_generics.iter()) {
                    Self::infer_generic_bindings_from_ty(p, a, bindings)?;
                }
                Ok(())
            }
            _ if Self::contains_generic_ty(pattern) => Self::generic_shape_error(pattern, actual),
            _ => Ok(()),
        }
    }

    pub(crate) fn instantiate_signature(
        signature: &hir::HirFunctionSignature,
        raw_args: &[hir::Expr],
    ) -> Result<hir::HirFunctionSignature, String> {
        let mut bindings = HashMap::new();
        for (param, arg) in signature.params.iter().zip(raw_args.iter()) {
            Self::infer_generic_bindings_from_ty(&param.ty, &arg.ty, &mut bindings)?;
        }
        if bindings.is_empty() {
            return Ok(signature.clone());
        }
        let mut instantiated = signature.clone();
        for param in &mut instantiated.params {
            param.ty = Self::substitute_generics_in_ty(&param.ty, &bindings);
        }
        instantiated.ret_type = Self::substitute_generics_in_ty(&instantiated.ret_type, &bindings);
        instantiated.effects = instantiated
            .effects
            .iter()
            .map(|ty| Self::substitute_generics_in_ty(ty, &bindings))
            .collect();
        Ok(instantiated)
    }

    pub(crate) fn instantiated_union_payload(
        &self,
        enum_name: &hir::OwnedPath,
        enum_generics: &[hir::Ty],
        variant: &str,
    ) -> Option<Option<Vec<hir::Ty>>> {
        let variants = self.union_variants.get(enum_name)?;
        let (_, payload) = variants.iter().find(|(name, _)| name == variant)?;
        let bindings = self.generic_bindings_for(enum_name, enum_generics);
        Some(payload.as_ref().map(|payload| {
            payload
                .iter()
                .map(|ty| Self::substitute_generics_in_ty(ty, &bindings))
                .collect()
        }))
    }

    fn generic_shape_error(pattern: &hir::Ty, actual: &hir::Ty) -> Result<(), String> {
        Err(format!(
            "Generic inference shape mismatch: expected {}, found {}",
            Typechecker::format_ty(pattern),
            Typechecker::format_ty(actual)
        ))
    }
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

    fn map_ty(key: hir::PrimitiveTy, value: hir::Ty) -> hir::Ty {
        hir::Ty::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    #[test]
    fn generic_inference_rejects_conflicting_bindings() {
        let mut bindings = HashMap::new();
        Typechecker::infer_generic_bindings_from_ty(
            &hir::Ty::Generic("T".to_string()),
            &i32_ty(),
            &mut bindings,
        )
        .unwrap();

        let err = Typechecker::infer_generic_bindings_from_ty(
            &hir::Ty::Generic("T".to_string()),
            &str_ty(),
            &mut bindings,
        )
        .unwrap_err();

        assert!(err.contains("Conflicting inference for generic `T`"));
    }

    #[test]
    fn generic_inference_rejects_structural_mismatches() {
        let mut bindings = HashMap::new();
        let err = Typechecker::infer_generic_bindings_from_ty(
            &option_ty(hir::Ty::Generic("T".to_string())),
            &i32_ty(),
            &mut bindings,
        )
        .unwrap_err();

        assert!(err.contains("Generic inference shape mismatch"));
    }

    #[test]
    fn generic_inference_rejects_map_key_mismatches() {
        let mut bindings = HashMap::new();
        let err = Typechecker::infer_generic_bindings_from_ty(
            &map_ty(hir::PrimitiveTy::I32, hir::Ty::Generic("V".to_string())),
            &map_ty(hir::PrimitiveTy::Str, str_ty()),
            &mut bindings,
        )
        .unwrap_err();

        assert!(err.contains("Map key type mismatch"));
    }
}
