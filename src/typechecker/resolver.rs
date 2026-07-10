use std::path::PathBuf;

use crate::ast_owned::{OwnedExpr, SpannedExpr};
use crate::hir;
use crate::token::SimpleSpan;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::errors::ItemContext;
use crate::typechecker::symbols::Symbol;

#[derive(Clone)]
pub(crate) struct ResolvedFunction {
    pub call_path: hir::OwnedPath,
    pub signature: hir::HirFunctionSignature,
    pub defined_in: PathBuf,
    pub decl_span: Option<SimpleSpan>,
}

pub(crate) struct ResolvedPathCall {
    pub display_name: String,
    pub adjusted_args: Vec<SpannedExpr>,
    pub function: Option<ResolvedFunction>,
}

pub(crate) enum ResolvedValue {
    Variable {
        name: String,
        ty: hir::Ty,
        is_mut: bool,
        initialized: bool,
        decl_span: Option<SimpleSpan>,
    },
    Function(ResolvedFunction),
    ImportedModule,
}

pub(crate) struct ResolvedField {
    pub owner: hir::OwnedPath,
    pub field: String,
    pub ty: hir::Ty,
}

pub(crate) struct ResolvedEnumConstructor {
    pub enum_path: hir::OwnedPath,
    pub enum_generics: Vec<hir::Ty>,
    pub variant_name: String,
    pub payload_types: Option<Vec<hir::Ty>>,
}

pub(crate) struct ResolvedVariantStruct {
    pub enum_path: hir::OwnedPath,
    pub field_defs: Vec<hir::HirField>,
}

pub(crate) struct ResolvedStructInit {
    pub path: hir::OwnedPath,
    pub field_defs: Vec<hir::HirField>,
}

pub(crate) struct ResolvedPatternVariant {
    pub path: hir::OwnedPath,
    pub payload_types: Option<Vec<hir::Ty>>,
    pub exists: bool,
}

pub(crate) struct ResolvedEffectOperation {
    pub effect_name: String,
    pub ret_ty: hir::Ty,
    pub param_tys: Vec<hir::Ty>,
}

pub(crate) struct ResolvedHandlerValue {
    pub body: hir::HirHandlerBody,
    pub effects: Vec<hir::Ty>,
}

pub(crate) struct ResolvedNominalType {
    pub kind: ResolvedNominalKind,
    pub canonical_path: hir::OwnedPath,
}

pub(crate) enum ResolvedNominalKind {
    Struct,
    Enum,
    Effect,
    Unsupported,
}

pub(crate) enum ResolveError {
    PrivateFunction { name: String },
}

pub(crate) enum TypeResolveError {
    PrivateType { name: String },
}

pub(crate) enum HandlerResolveError {
    EmptyPath,
    NotHandlerValue { name: String },
    UnknownHandler { name: String },
}

pub(crate) enum VariantResolveError {
    AmbiguousVariantConstructor {
        name: String,
        candidates: Vec<hir::OwnedPath>,
    },
    UnknownVariant {
        enum_path: hir::OwnedPath,
        variant: String,
    },
}

impl Typechecker {
    pub(crate) fn resolve_value_path(
        &self,
        path: &[String],
        context: &ItemContext,
    ) -> Result<Option<ResolvedValue>, ResolveError> {
        let Some(name) = path.last().cloned() else {
            return Ok(None);
        };
        match self.lookup_symbol(&name) {
            Some(Symbol::Variable {
                ty,
                is_mut,
                initialized,
                decl_span,
            }) => Ok(Some(ResolvedValue::Variable {
                name,
                ty: ty.clone(),
                is_mut: *is_mut,
                initialized: *initialized,
                decl_span: *decl_span,
            })),
            Some(Symbol::Function { .. }) => self
                .resolve_visible_function_path(path, context)
                .map(|function| function.map(ResolvedValue::Function)),
            None => {
                let is_imported_module = self
                    .imports_by_file
                    .get(&context.path)
                    .map(|names| names.contains(&name))
                    .unwrap_or(false);
                Ok(is_imported_module.then_some(ResolvedValue::ImportedModule))
            }
        }
    }

    pub(crate) fn resolve_field_function(
        &self,
        receiver: &OwnedExpr,
        field: &str,
        context: &ItemContext,
    ) -> Result<Option<ResolvedFunction>, ResolveError> {
        let OwnedExpr::Path(path) = receiver else {
            return Ok(None);
        };
        let Some(base) = path.last() else {
            return Ok(None);
        };
        if self.lookup_symbol(base).is_some() {
            return Ok(None);
        }
        self.resolve_visible_function_path(&[field.to_string()], context)
    }

    pub(crate) fn resolve_struct_field(
        &self,
        receiver_ty: &hir::Ty,
        field: &str,
    ) -> Option<ResolvedField> {
        let hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) = receiver_ty else {
            return None;
        };
        let ty = match self.type_definitions.get(name) {
            Some(hir::Item::Struct(def)) => def
                .fields
                .iter()
                .find(|candidate| candidate.name == field)
                .map(|candidate| candidate.ty.clone())?,
            _ => return None,
        };
        Some(ResolvedField {
            owner: name.clone(),
            field: field.to_string(),
            ty,
        })
    }

    pub(crate) fn resolve_enum_constructor(
        &self,
        variant_name: &str,
    ) -> Result<Option<ResolvedEnumConstructor>, VariantResolveError> {
        let matches = self.find_union_variant_matches(variant_name);
        if matches.len() > 1 {
            return Err(VariantResolveError::AmbiguousVariantConstructor {
                name: variant_name.to_string(),
                candidates: matches
                    .into_iter()
                    .map(|(enum_path, _)| enum_path)
                    .collect(),
            });
        }
        Ok(matches
            .into_iter()
            .next()
            .map(|(enum_path, payload_types)| ResolvedEnumConstructor {
                enum_path,
                enum_generics: vec![],
                variant_name: variant_name.to_string(),
                payload_types,
            }))
    }

    pub(crate) fn resolve_expected_enum_constructor(
        &self,
        enum_path: &hir::OwnedPath,
        enum_generics: &[hir::Ty],
        variant_name: &str,
    ) -> Option<ResolvedEnumConstructor> {
        self.instantiated_union_payload(enum_path, enum_generics, variant_name)
            .map(|payload_types| ResolvedEnumConstructor {
                enum_path: enum_path.clone(),
                enum_generics: enum_generics.to_vec(),
                variant_name: variant_name.to_string(),
                payload_types,
            })
    }

    pub(crate) fn resolve_variant_struct_init(
        &self,
        path: &[String],
    ) -> Result<Option<ResolvedVariantStruct>, VariantResolveError> {
        let Some((variant, enum_path_parts)) = path.split_last() else {
            return Ok(None);
        };
        if enum_path_parts.is_empty() {
            return Ok(None);
        }
        let enum_path = enum_path_parts.to_vec();
        let Some(variants) = self.union_variants.get(&enum_path) else {
            return Ok(None);
        };
        let Some((_, payload)) = variants.iter().find(|(name, _)| name == variant) else {
            return Err(VariantResolveError::UnknownVariant {
                enum_path,
                variant: variant.clone(),
            });
        };
        let payload_types = payload.clone().unwrap_or_default();
        let field_defs = if let [
            hir::Ty::Adt(hir::AdtTy::Struct {
                name: payload_struct,
                ..
            }),
        ] = payload_types.as_slice()
        {
            match self.type_definitions.get(payload_struct) {
                Some(hir::Item::Struct(struct_def)) => struct_def.fields.clone(),
                _ => vec![],
            }
        } else {
            vec![]
        };
        Ok(Some(ResolvedVariantStruct {
            enum_path,
            field_defs,
        }))
    }

    pub(crate) fn resolve_struct_init(
        &self,
        path: &[String],
        context: &ItemContext,
    ) -> Result<Option<ResolvedStructInit>, TypeResolveError> {
        let Some(resolved) = self.resolve_nominal_type_path(&path.to_vec(), context)? else {
            return Ok(None);
        };
        match self.type_definitions.get(&resolved.canonical_path) {
            Some(hir::Item::Struct(struct_def)) => Ok(Some(ResolvedStructInit {
                path: resolved.canonical_path,
                field_defs: struct_def.fields.clone(),
            })),
            _ => Ok(None),
        }
    }

    pub(crate) fn resolve_pattern_variant(
        &self,
        scrutinee_ty: &hir::Ty,
        variant_path: &[String],
    ) -> Option<ResolvedPatternVariant> {
        let variant_name = variant_path.last()?.clone();
        match scrutinee_ty {
            hir::Ty::Adt(hir::AdtTy::Enum { name, generics }) => {
                let mut path = name.clone();
                path.push(variant_name.clone());
                match self.instantiated_union_payload(name, generics, &variant_name) {
                    Some(payload_types) => Some(ResolvedPatternVariant {
                        path,
                        payload_types,
                        exists: true,
                    }),
                    None => Some(ResolvedPatternVariant {
                        path,
                        payload_types: None,
                        exists: false,
                    }),
                }
            }
            _ => Some(ResolvedPatternVariant {
                path: variant_path.to_vec(),
                payload_types: None,
                exists: false,
            }),
        }
    }

    pub(crate) fn resolve_effect_operation(
        &self,
        path: &hir::OwnedPath,
    ) -> Option<ResolvedEffectOperation> {
        if path.len() != 2 {
            return None;
        }
        let effect_name = path[0].clone();
        let effect_path = vec![effect_name.clone()];
        let op_name = &path[1];
        match self.type_definitions.get(&effect_path) {
            Some(hir::Item::Effect(def)) => def
                .operations
                .iter()
                .find(|sig| &sig.name == op_name)
                .map(|sig| ResolvedEffectOperation {
                    effect_name,
                    ret_ty: sig.ret_type.clone(),
                    param_tys: sig.params.iter().map(|p| p.ty.clone()).collect(),
                }),
            _ => None,
        }
    }

    pub(crate) fn resolve_effect_type_name(&self, name: &str) -> Option<hir::Ty> {
        let path = vec![name.to_string()];
        self.resolve_effect_type_path(&path)
    }

    pub(crate) fn resolve_effect_type_path(&self, path: &hir::OwnedPath) -> Option<hir::Ty> {
        match self.type_definitions.get(path) {
            Some(hir::Item::Effect(_)) => Some(hir::Ty::Adt(hir::AdtTy::Effect {
                name: path.clone(),
                generics: vec![],
            })),
            _ => None,
        }
    }

    pub(crate) fn resolve_effect_operations(
        &self,
        path: &hir::OwnedPath,
    ) -> Option<Vec<hir::HirFunctionSignature>> {
        match self.type_definitions.get(path) {
            Some(hir::Item::Effect(def)) => Some(def.operations.clone()),
            _ => None,
        }
    }

    pub(crate) fn resolve_registered_handler_path(
        &self,
        path: &[String],
    ) -> Option<ResolvedHandlerValue> {
        let name = path.last()?;
        let effects = self.handler_values.get(name)?.clone();
        Some(ResolvedHandlerValue {
            body: self.resolved_handler_body(path),
            effects,
        })
    }

    pub(crate) fn resolve_handler_body_path(
        &self,
        path: &[String],
    ) -> Result<ResolvedHandlerValue, HandlerResolveError> {
        let Some(name) = path.last() else {
            return Err(HandlerResolveError::EmptyPath);
        };
        if let Some(effects) = self.handler_values.get(name).cloned() {
            return Ok(ResolvedHandlerValue {
                body: self.resolved_handler_body(path),
                effects,
            });
        }
        match self.lookup_symbol(name) {
            Some(Symbol::Variable {
                ty: hir::Ty::Handler { effects },
                ..
            }) => Ok(ResolvedHandlerValue {
                body: hir::HirHandlerBody::Path(path.to_vec()),
                effects: effects.clone(),
            }),
            Some(Symbol::Variable { .. }) => {
                Err(HandlerResolveError::NotHandlerValue { name: name.clone() })
            }
            _ => Err(HandlerResolveError::UnknownHandler { name: name.clone() }),
        }
    }

    fn resolved_handler_body(&self, path: &[String]) -> hir::HirHandlerBody {
        let Some(name) = path.last() else {
            return hir::HirHandlerBody::Path(vec![]);
        };
        if let Some((base, handlers)) = self.handler_aliases.get(name).cloned() {
            hir::HirHandlerBody::Composed {
                base: Box::new(hir::HirHandlerBody::Path(vec![base])),
                handlers: handlers
                    .into_iter()
                    .filter(|name| !name.is_empty())
                    .map(|name| hir::HirHandlerBody::Path(vec![name]))
                    .collect(),
            }
        } else {
            hir::HirHandlerBody::Path(path.to_vec())
        }
    }

    pub(crate) fn resolve_nominal_type_path(
        &self,
        path: &hir::OwnedPath,
        context: &ItemContext,
    ) -> Result<Option<ResolvedNominalType>, TypeResolveError> {
        let mut candidates = Vec::new();
        if self.type_definitions.contains_key(path) {
            candidates.push(path.clone());
        }
        if let Some((first, rest)) = path.split_first()
            && !rest.is_empty()
        {
            if let Some(module) = self
                .import_alias_map
                .get(&context.path)
                .and_then(|aliases| aliases.get(first))
            {
                let mut expanded = module.clone();
                expanded.extend_from_slice(rest);
                if self.type_definitions.contains_key(&expanded) {
                    candidates.push(expanded);
                }
            }
        }
        if path.len() == 1 {
            let mut local =
                crate::typechecker::checker::Typechecker::canonical_module_path(&context.path);
            local.extend(path.iter().cloned());
            if self.type_definitions.contains_key(&local) {
                candidates.push(local);
            }
        }
        candidates.sort();
        candidates.dedup();
        let Some(canonical_path) = candidates.into_iter().next() else {
            return Ok(None);
        };
        let item = &self.type_definitions[&canonical_path];
        if let Some((defined_in, is_public)) = self.type_definition_meta.get(&canonical_path) {
            if !*is_public && defined_in != &context.path {
                return Err(TypeResolveError::PrivateType {
                    name: path.join("::"),
                });
            }
        }
        Ok(Some(ResolvedNominalType {
            kind: match item {
                hir::Item::Struct(_) => ResolvedNominalKind::Struct,
                hir::Item::Enum(_) => ResolvedNominalKind::Enum,
                hir::Item::Effect(_) => ResolvedNominalKind::Effect,
                _ => ResolvedNominalKind::Unsupported,
            },
            canonical_path,
        }))
    }

    fn find_union_variant_matches(
        &self,
        variant: &str,
    ) -> Vec<(hir::OwnedPath, Option<Vec<hir::Ty>>)> {
        let mut matches = Vec::new();
        for (union_path, variants) in &self.union_variants {
            if let Some((_, payload)) = variants.iter().find(|(name, _)| name == variant).cloned() {
                matches.push((union_path.clone(), payload));
            }
        }
        matches.sort_by(|(left, _), (right, _)| left.cmp(right));
        matches
    }

    pub(crate) fn resolve_path_call(
        &self,
        path: &[String],
        args: &[SpannedExpr],
        context: &ItemContext,
    ) -> Result<ResolvedPathCall, ResolveError> {
        let display_name = path.last().cloned().unwrap_or_default();
        let (lookup_path, adjusted_args) =
            self.normalize_module_qualified_call(path, args, context);
        let function = self.resolve_visible_function_path(&lookup_path, context)?;
        Ok(ResolvedPathCall {
            display_name,
            adjusted_args,
            function,
        })
    }

    pub(crate) fn resolve_method_function_candidates(
        &self,
        method: &str,
        receiver_ty: &hir::Ty,
        context: &ItemContext,
    ) -> Result<Vec<ResolvedFunction>, ResolveError> {
        let mut candidates: Vec<hir::OwnedPath> = vec![vec![method.to_string()]];
        if let hir::Ty::Adt(hir::AdtTy::Struct {
            name: type_path, ..
        })
        | hir::Ty::Adt(hir::AdtTy::Enum {
            name: type_path, ..
        }) = receiver_ty
        {
            if type_path.len() > 1 {
                let module_path: Vec<String> = type_path
                    .iter()
                    .cloned()
                    .take(type_path.len().saturating_sub(1))
                    .collect();
                if let Some(alias_map) = self.import_alias_map.get(&context.path) {
                    for (alias, mod_path) in alias_map.iter() {
                        if mod_path == &module_path {
                            candidates.push(vec![alias.clone(), method.to_string()]);
                        }
                    }
                }
            }
        }

        let mut resolved = Vec::new();
        for candidate in candidates {
            if let Some(function) = self.resolve_visible_function_path(&candidate, context)? {
                resolved.push(function);
            }
        }
        Ok(resolved)
    }

    fn normalize_module_qualified_call(
        &self,
        path: &[String],
        args: &[SpannedExpr],
        context: &ItemContext,
    ) -> (hir::OwnedPath, Vec<SpannedExpr>) {
        let mut adjusted_args = args.to_vec();
        if let Some(first) = args.first() {
            if let crate::ast_owned::OwnedExpr::Path(first_path) = &first.item {
                if first_path.len() == 1 {
                    let base = first_path.last().expect("single-segment path");
                    let is_import_alias = self
                        .imports_by_file
                        .get(&context.path)
                        .map(|names| names.contains(base))
                        .unwrap_or(false);
                    let is_value_symbol = self.lookup_symbol(base).is_some();
                    if is_import_alias && !is_value_symbol {
                        adjusted_args = args.iter().skip(1).cloned().collect();
                    }
                }
            }
        }
        (path.to_vec(), adjusted_args)
    }

    fn resolve_visible_function_path(
        &self,
        path: &[String],
        context: &ItemContext,
    ) -> Result<Option<ResolvedFunction>, ResolveError> {
        let lookup_names = self.function_lookup_names(path);
        for lookup_name in lookup_names {
            let Some(Symbol::Function {
                signature,
                is_public,
                defined_in,
                decl_span,
            }) = self.lookup_symbol(&lookup_name)
            else {
                continue;
            };
            if !*is_public && *defined_in != context.path {
                return Err(ResolveError::PrivateFunction { name: lookup_name });
            }
            return Ok(Some(ResolvedFunction {
                call_path: path.to_vec(),
                signature: signature.clone(),
                defined_in: defined_in.clone(),
                decl_span: *decl_span,
            }));
        }
        Ok(None)
    }

    fn function_lookup_names(&self, path: &[String]) -> Vec<String> {
        match path {
            [] => vec![],
            [name] => vec![name.clone()],
            [alias, name] => {
                let qualified = format!("{}::{}", alias, name);
                vec![qualified, name.clone()]
            }
            _ => vec![path.join("::")],
        }
    }
}
