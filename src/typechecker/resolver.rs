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

pub(crate) enum ResolveError {
    PrivateFunction { name: String },
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
        let ty = self.lookup_struct_field_type(name, field)?.clone();
        Some(ResolvedField {
            owner: name.clone(),
            field: field.to_string(),
            ty,
        })
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
