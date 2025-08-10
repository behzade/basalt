use std::path::PathBuf;

use crate::hir;
use crate::typechecker::errors::{ItemContext, TypeError};
use crate::typechecker::symbols::Symbol;
use crate::typechecker::checker::Typechecker;
use crate::ast_owned::{OwnedItem, OwnedItemWithSpan, OwnedTypeAliasBody};

impl Typechecker {
    pub(crate) fn register_top_level_item(&mut self, item: &OwnedItemWithSpan) {
        match &item.item {
            OwnedItem::Fn(func) => {
                let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                // Disallow duplicate function names globally (including multiple 'main')
                if self.top_level_functions.contains_key(&func.name) {
                    self.errors.push(TypeError { message: format!("Duplicate function '{}'", func.name), context: ctx.clone() });
                    return;
                }
                let mut params: Vec<hir::HirParam> = Vec::new();
                let mut has_error = false;
                for (name_opt, ty) in &func.params {
                    match self.resolve_type(ty, ctx.clone()) {
                        Ok(t) => params.push(hir::HirParam { name: name_opt.clone().unwrap_or("_".to_string()), ty: t, span: None }),
                        Err(_) => has_error = true,
                    }
                }
                let ret_ty = match &func.ret_type {
                    Some(rt) => self.resolve_type(rt, ctx.clone()).unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit)),
                    None => hir::Ty::Special(hir::SpecialTy::Unit),
                };
                if !has_error {
                    let signature = hir::HirFunctionSignature { name: func.name.clone(), params, ret_type: ret_ty, effects: vec![] };
                    self.top_level_functions.insert(func.name.clone(), (signature.clone(), func.is_public, ctx.path.clone()));
                    self.add_symbol_to_current_scope(
                        func.name.clone(),
                        Symbol::Function { signature, is_public: func.is_public, defined_in: ctx.path.clone(), decl_span: Some(item.span) },
                    );
                }
            }
            OwnedItem::TypeAlias(ta) => {
                match &ta.aliased {
                    OwnedTypeAliasBody::Record(fields) => {
                        let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                        let mut lowered_fields: Vec<hir::HirField> = Vec::new();
                        for (name, ty) in fields {
                            if let Ok(t) = self.resolve_type(ty, ctx.clone()) {
                                lowered_fields.push(hir::HirField { name: name.clone(), ty: t, name_span: None });
                            }
                        }
                        let def = hir::HirStructDef { name: ta.name.clone(), fields: lowered_fields, is_public: ta.is_public, defined_in: ctx.path.clone(), span: item.span };
                        let path = vec![ta.name.clone()];
                        self.type_definitions.insert(path.clone(), hir::Item::Struct(def));
                        self.type_definition_meta.insert(path, (ctx.path.clone(), ta.is_public));
                    }
                    OwnedTypeAliasBody::Union(variants) => {
                        let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                        let mut lowered_variants: Vec<hir::HirEnumVariant> = Vec::new();
                        for (vname, payload) in variants {
                            let lowered_payload = match payload {
                                Some(t) => match self.resolve_type(t, ctx.clone()) {
                                    Ok(h) => Some(vec![h]),
                                    Err(_) => None,
                                },
                                None => None,
                            };
                            lowered_variants.push(hir::HirEnumVariant { name: vname.clone(), payload: lowered_payload, name_span: None });
                        }
                        let def = hir::HirEnumDef { name: ta.name.clone(), variants: lowered_variants.clone(), is_public: ta.is_public, defined_in: ctx.path.clone(), span: item.span };
                        let path = vec![ta.name.clone()];
                        self.type_definitions.insert(path.clone(), hir::Item::Enum(def));
                        self.type_definition_meta.insert(path.clone(), (ctx.path.clone(), ta.is_public));
                        let uv: Vec<(String, Option<Vec<hir::Ty>>)> = lowered_variants.iter().map(|v| (v.name.clone(), v.payload.clone())).collect();
                        self.union_variants.insert(path, uv);
                    }
                    OwnedTypeAliasBody::Type(_) => {}
                }
            }
            OwnedItem::Effect(eff) => {
                let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                let mut ops: Vec<hir::HirFunctionSignature> = Vec::new();
                for op in &eff.operations {
                    let mut params: Vec<hir::HirParam> = Vec::new();
                    for p in &op.params {
                        if let Ok(t) = self.resolve_type(p, ctx.clone()) {
                            params.push(hir::HirParam { name: "_".to_string(), ty: t, span: None });
                        }
                    }
                    let ret_ty = self
                        .resolve_type(&op.ret_type, ctx.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    ops.push(hir::HirFunctionSignature { name: op.name.clone(), params, ret_type: ret_ty, effects: vec![] });
                }
                let def = hir::HirEffectDef { name: eff.name.clone(), operations: ops, is_public: eff.is_public, defined_in: ctx.path.clone(), span: item.span };
                let path = vec![eff.name.clone()];
                self.type_definitions.insert(path.clone(), hir::Item::Effect(def));
                self.type_definition_meta.insert(path, (ctx.path.clone(), eff.is_public));
            }
            _ => {}
        }
    }

    pub(crate) fn register_top_level_item_with_file(&mut self, item: &OwnedItemWithSpan, file: &PathBuf) {
        let mut item_clone = item.clone();
        let ctx = ItemContext { span: item.span, path: file.clone() };
        match &mut item_clone.item {
            OwnedItem::Fn(func) => {
                // Disallow duplicate function names globally (including multiple 'main')
                if self.top_level_functions.contains_key(&func.name) {
                    self.errors.push(TypeError { message: format!("Duplicate function '{}'", func.name), context: ctx.clone() });
                    return;
                }
                let mut params: Vec<hir::HirParam> = Vec::new();
                let mut has_error = false;
                for (name_opt, ty) in &func.params {
                    match self.resolve_type(ty, ctx.clone()) {
                        Ok(t) => params.push(hir::HirParam { name: name_opt.clone().clone().unwrap_or("_".to_string()), ty: t, span: None }),
                        Err(_) => has_error = true,
                    }
                }
                let ret_ty = match &func.ret_type {
                    Some(rt) => self.resolve_type(rt, ctx.clone()).unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit)),
                    None => hir::Ty::Special(hir::SpecialTy::Unit),
                };
                if !has_error {
                    let signature = hir::HirFunctionSignature { name: func.name.clone(), params, ret_type: ret_ty, effects: vec![] };
                    self.top_level_functions.insert(func.name.clone(), (signature.clone(), func.is_public, ctx.path.clone()));
                    self.add_symbol_to_current_scope(func.name.clone(), Symbol::Function { signature, is_public: func.is_public, defined_in: ctx.path.clone(), decl_span: Some(item.span) });
                }
            }
            _ => {
                let saved = ItemContext { span: item.span, path: file.clone() };
                let owned = OwnedItemWithSpan { item: item.item.clone(), span: saved.span };
                self.register_top_level_item(&owned);
            }
        }
    }

    pub(crate) fn register_builtin_functions(&mut self) {
        let signature = hir::HirFunctionSignature {
            name: "len".to_string(),
            params: vec![hir::HirParam { name: "s".to_string(), ty: hir::Ty::Primitive(hir::PrimitiveTy::Str), span: None }],
            ret_type: hir::Ty::Primitive(hir::PrimitiveTy::I32),
            effects: vec![],
        };
        self.add_symbol_to_current_scope(
            "len".to_string(),
            Symbol::Function { signature, is_public: true, defined_in: PathBuf::from("<builtin>"), decl_span: None },
        );
    }
}


