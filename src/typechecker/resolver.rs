use std::path::PathBuf;

use crate::ast_owned::SpannedExpr;
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

pub(crate) enum ResolveError {
    PrivateFunction { name: String },
}

impl Typechecker {
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
        let lookup_names = self.function_lookup_names(path, context);
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

    fn function_lookup_names(&self, path: &[String], context: &ItemContext) -> Vec<String> {
        match path {
            [] => vec![],
            [name] => vec![name.clone()],
            [alias, name] => {
                let qualified = format!("{}::{}", alias, name);
                let alias_is_import = self
                    .import_alias_map
                    .get(&context.path)
                    .and_then(|aliases| aliases.get(alias))
                    .is_some();
                if alias_is_import {
                    vec![qualified]
                } else {
                    vec![qualified, name.clone()]
                }
            }
            _ => vec![path.join("::")],
        }
    }
}
