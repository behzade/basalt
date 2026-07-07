use std::path::PathBuf;

use crate::ast_owned::{OwnedExpr, OwnedItem, OwnedItemWithSpan, OwnedStmt, OwnedTypeAliasBody};
use crate::hir;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::errors::{ItemContext, TypeError};
use crate::typechecker::symbols::Symbol;

impl Typechecker {
    fn effect_set_difference(left: &[hir::Ty], right: &[hir::Ty]) -> Vec<hir::Ty> {
        left.iter()
            .filter(|effect| {
                !right
                    .iter()
                    .any(|handled| Self::same_effect_name(effect, handled))
            })
            .cloned()
            .collect()
    }

    pub(crate) fn same_effect_name(a: &hir::Ty, b: &hir::Ty) -> bool {
        match (a, b) {
            (
                hir::Ty::Adt(hir::AdtTy::Effect { name: a, .. }),
                hir::Ty::Adt(hir::AdtTy::Effect { name: b, .. }),
            ) => a == b,
            (hir::Ty::Generic(a), hir::Ty::Generic(b)) => a == b,
            (hir::Ty::Adt(hir::AdtTy::Effect { name, .. }), hir::Ty::Generic(g))
            | (hir::Ty::Generic(g), hir::Ty::Adt(hir::AdtTy::Effect { name, .. })) => {
                name.last().map(|n| n == g).unwrap_or(false)
            }
            _ => false,
        }
    }

    fn resolve_effect_names(
        &mut self,
        effect_names: &[String],
        context: ItemContext,
    ) -> Option<Vec<hir::Ty>> {
        let mut effects = Vec::new();
        for eff_name in effect_names {
            let path = vec![eff_name.clone()];
            if let Some(hir::Item::Effect(_)) = self.type_definitions.get(&path) {
                effects.push(hir::Ty::Adt(hir::AdtTy::Effect {
                    name: path,
                    generics: vec![],
                }));
            } else {
                self.errors.push(TypeError {
                    message: format!("Unknown effect `{}`", eff_name),
                    context: context.clone(),
                });
                return None;
            }
        }
        Some(effects)
    }

    fn register_handler_value(&mut self, name: &str, effects: &[String], context: ItemContext) {
        if let Some(effects) = self.resolve_effect_names(effects, context) {
            self.handler_values.insert(name.to_string(), effects);
        }
    }

    fn register_handler_alias_stmt(&mut self, item: &OwnedItemWithSpan, context: ItemContext) {
        let OwnedItem::Stmt(stmt) = &item.item else {
            return;
        };
        let OwnedStmt::Let {
            name,
            value: Some(value),
            ..
        } = &stmt.item
        else {
            return;
        };
        let OwnedExpr::Handle { body, handler } = &value.item else {
            return;
        };
        let OwnedExpr::Path(body_path) = &body.item else {
            return;
        };
        let Some(body_name) = body_path.last() else {
            return;
        };
        let Some(body_effects) = self.handler_values.get(body_name).cloned() else {
            return;
        };
        let handler_names = match handler {
            crate::ast_owned::OwnedHandlerBody::Path(path) => {
                vec![path.last().cloned().unwrap_or_default()]
            }
            crate::ast_owned::OwnedHandlerBody::Inline(_) => vec![],
        };
        let handler_effects = if handler_names.is_empty() {
            Some(vec![])
        } else {
            let mut effects = vec![];
            for handler_name in &handler_names {
                let Some(handler_effects) = self.handler_values.get(handler_name) else {
                    self.errors.push(TypeError {
                        message: format!("Unknown handler `{}`", handler_name),
                        context,
                    });
                    return;
                };
                effects.extend(handler_effects.clone());
            }
            Some(effects)
        };
        let Some(handler_effects) = handler_effects else {
            self.errors.push(TypeError {
                message: format!("Unknown handler `{}`", body_name),
                context,
            });
            return;
        };
        self.handler_aliases
            .insert(name.clone(), (body_name.clone(), handler_names));
        self.handler_values.insert(
            name.clone(),
            Self::effect_set_difference(&body_effects, &handler_effects),
        );
    }

    pub(crate) fn register_top_level_item(&mut self, item: &OwnedItemWithSpan) {
        match &item.item {
            OwnedItem::Fn(func) => {
                let ctx = ItemContext {
                    span: item.span,
                    path: PathBuf::from("<global>"),
                };
                // Allow duplicate function names across modules; resolution can be qualified.
                let mut params: Vec<hir::HirParam> = Vec::new();
                let mut has_error = false;
                for (is_mut, name_opt, ty) in &func.params {
                    match self.resolve_type(ty, ctx.clone()) {
                        Ok(t) => params.push(hir::HirParam {
                            name: name_opt.clone().unwrap_or("_".to_string()),
                            ty: t,
                            is_mut: *is_mut,
                            span: None,
                        }),
                        Err(_) => has_error = true,
                    }
                }
                let ret_ty = match &func.ret_type {
                    Some(rt) => self
                        .resolve_type(rt, ctx.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit)),
                    None => hir::Ty::Special(hir::SpecialTy::Unit),
                };
                let Some(effects) = self.resolve_effect_names(&func.effects, ctx.clone()) else {
                    return;
                };
                if !has_error {
                    let signature = hir::HirFunctionSignature {
                        name: func.name.clone(),
                        params,
                        ret_type: ret_ty,
                        effects,
                    };
                    self.top_level_functions.insert(
                        func.name.clone(),
                        (signature.clone(), func.is_public, ctx.path.clone()),
                    );
                    self.add_symbol_to_current_scope(
                        func.name.clone(),
                        Symbol::Function {
                            signature,
                            is_public: func.is_public,
                            defined_in: ctx.path.clone(),
                            decl_span: Some(item.span),
                        },
                    );
                }
            }
            OwnedItem::TypeAlias(ta) => match &ta.aliased {
                OwnedTypeAliasBody::Union(variants) => {
                    let ctx = ItemContext {
                        span: item.span,
                        path: PathBuf::from("<global>"),
                    };
                    let mut lowered_variants: Vec<hir::HirEnumVariant> = Vec::new();
                    for (vname, payload_ty) in variants {
                        let lowered_payload = match self.resolve_type(payload_ty, ctx.clone()) {
                            Ok(h) => Some(vec![h]),
                            Err(_) => None,
                        };
                        lowered_variants.push(hir::HirEnumVariant {
                            name: vname.clone(),
                            payload: lowered_payload,
                            name_span: None,
                        });
                    }
                    let def = hir::HirEnumDef {
                        name: ta.name.clone(),
                        variants: lowered_variants.clone(),
                        is_public: ta.is_public,
                        defined_in: ctx.path.clone(),
                        span: item.span,
                        context_id: None,
                    };
                    let path = vec![ta.name.clone()];
                    self.type_alias_generics
                        .insert(path.clone(), ta.generics.clone());
                    self.type_definitions
                        .insert(path.clone(), hir::Item::Enum(def));
                    self.type_definition_meta
                        .insert(path.clone(), (ctx.path.clone(), ta.is_public));
                    let uv: Vec<(String, Option<Vec<hir::Ty>>)> = lowered_variants
                        .iter()
                        .map(|v| (v.name.clone(), v.payload.clone()))
                        .collect();
                    self.union_variants.insert(path, uv);
                }
                OwnedTypeAliasBody::Type(_) => {}
            },
            OwnedItem::Struct(s) => {
                let ctx = ItemContext {
                    span: item.span,
                    path: PathBuf::from("<global>"),
                };
                let mut lowered_fields: Vec<hir::HirField> = Vec::new();
                for (name, ty) in &s.fields {
                    if let Ok(t) = self.resolve_type(ty, ctx.clone()) {
                        lowered_fields.push(hir::HirField {
                            name: name.clone(),
                            ty: t,
                            name_span: None,
                        });
                    }
                }
                let def = hir::HirStructDef {
                    name: s.name.clone(),
                    fields: lowered_fields,
                    is_public: s.is_public,
                    defined_in: ctx.path.clone(),
                    span: item.span,
                    context_id: None,
                };
                let path = vec![s.name.clone()];
                self.type_definitions
                    .insert(path.clone(), hir::Item::Struct(def));
                self.type_definition_meta
                    .insert(path, (ctx.path.clone(), s.is_public));
            }
            OwnedItem::Effect(eff) => {
                let ctx = ItemContext {
                    span: item.span,
                    path: PathBuf::from("<global>"),
                };
                let mut ops: Vec<hir::HirFunctionSignature> = Vec::new();
                for op in &eff.operations {
                    let mut params: Vec<hir::HirParam> = Vec::new();
                    for p in &op.params {
                        if let Ok(t) = self.resolve_type(p, ctx.clone()) {
                            params.push(hir::HirParam {
                                name: "_".to_string(),
                                ty: t,
                                is_mut: false,
                                span: None,
                            });
                        }
                    }
                    let ret_ty = self
                        .resolve_type(&op.ret_type, ctx.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    ops.push(hir::HirFunctionSignature {
                        name: op.name.clone(),
                        params,
                        ret_type: ret_ty,
                        effects: vec![],
                    });
                }
                let def = hir::HirEffectDef {
                    name: eff.name.clone(),
                    operations: ops,
                    is_public: eff.is_public,
                    defined_in: ctx.path.clone(),
                    span: item.span,
                };
                let path = vec![eff.name.clone()];
                self.type_definitions
                    .insert(path.clone(), hir::Item::Effect(def));
                self.type_definition_meta
                    .insert(path, (ctx.path.clone(), eff.is_public));
            }
            OwnedItem::Handler(h) => {
                let ctx = ItemContext {
                    span: item.span,
                    path: PathBuf::from("<global>"),
                };
                self.register_handler_value(&h.name, &h.effects, ctx);
            }
            OwnedItem::Stmt(_) => {
                self.register_handler_alias_stmt(
                    item,
                    ItemContext {
                        span: item.span,
                        path: PathBuf::from("<global>"),
                    },
                );
            }
            _ => {}
        }
    }

    pub(crate) fn register_top_level_item_with_file(
        &mut self,
        item: &OwnedItemWithSpan,
        file: &PathBuf,
    ) {
        let mut item_clone = item.clone();
        let ctx = ItemContext {
            span: item.span,
            path: file.clone(),
        };
        match &mut item_clone.item {
            OwnedItem::Fn(func) => {
                // Disallow duplicate function names within the same file/module
                if let Some(Symbol::Function { defined_in, .. }) = self.lookup_symbol(&func.name) {
                    if defined_in == &ctx.path {
                        self.errors.push(TypeError {
                            message: format!("Duplicate function '{}'", func.name),
                            context: ctx.clone(),
                        });
                        return;
                    }
                }
                let mut params: Vec<hir::HirParam> = Vec::new();
                let mut has_error = false;
                for (is_mut, name_opt, ty) in &func.params {
                    match self.resolve_type(ty, ctx.clone()) {
                        Ok(t) => params.push(hir::HirParam {
                            name: name_opt.clone().clone().unwrap_or("_".to_string()),
                            ty: t,
                            is_mut: *is_mut,
                            span: None,
                        }),
                        Err(_) => has_error = true,
                    }
                }
                let ret_ty = match &func.ret_type {
                    Some(rt) => self
                        .resolve_type(rt, ctx.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit)),
                    None => hir::Ty::Special(hir::SpecialTy::Unit),
                };
                let Some(effects) = self.resolve_effect_names(&func.effects, ctx.clone()) else {
                    return;
                };
                if !has_error {
                    let signature = hir::HirFunctionSignature {
                        name: func.name.clone(),
                        params,
                        ret_type: ret_ty,
                        effects,
                    };
                    self.top_level_functions.insert(
                        func.name.clone(),
                        (signature.clone(), func.is_public, ctx.path.clone()),
                    );
                    self.add_symbol_to_current_scope(
                        func.name.clone(),
                        Symbol::Function {
                            signature,
                            is_public: func.is_public,
                            defined_in: ctx.path.clone(),
                            decl_span: Some(item.span),
                        },
                    );
                }
            }
            OwnedItem::TypeAlias(ta) => match &ta.aliased {
                OwnedTypeAliasBody::Union(variants) => {
                    let ctx2 = ItemContext {
                        span: item.span,
                        path: file.clone(),
                    };
                    let mut lowered_variants: Vec<hir::HirEnumVariant> = Vec::new();
                    for (vname, payload_ty) in variants {
                        let lowered_payload = match self.resolve_type(payload_ty, ctx2.clone()) {
                            Ok(h) => Some(vec![h]),
                            Err(_) => None,
                        };
                        lowered_variants.push(hir::HirEnumVariant {
                            name: vname.clone(),
                            payload: lowered_payload,
                            name_span: None,
                        });
                    }
                    let def = hir::HirEnumDef {
                        name: ta.name.clone(),
                        variants: lowered_variants.clone(),
                        is_public: ta.is_public,
                        defined_in: ctx2.path.clone(),
                        span: item.span,
                        context_id: None,
                    };
                    let path = vec![ta.name.clone()];
                    self.type_alias_generics
                        .insert(path.clone(), ta.generics.clone());
                    self.type_definitions
                        .insert(path.clone(), hir::Item::Enum(def));
                    self.type_definition_meta
                        .insert(path.clone(), (ctx2.path.clone(), ta.is_public));
                    let uv: Vec<(String, Option<Vec<hir::Ty>>)> = lowered_variants
                        .iter()
                        .map(|v| (v.name.clone(), v.payload.clone()))
                        .collect();
                    self.union_variants.insert(path, uv);
                }
                OwnedTypeAliasBody::Type(_) => {}
            },
            OwnedItem::Struct(s) => {
                let ctx2 = ItemContext {
                    span: item.span,
                    path: file.clone(),
                };
                let mut lowered_fields: Vec<hir::HirField> = Vec::new();
                for (name, ty) in &s.fields {
                    if let Ok(t) = self.resolve_type(ty, ctx2.clone()) {
                        lowered_fields.push(hir::HirField {
                            name: name.clone(),
                            ty: t,
                            name_span: None,
                        });
                    }
                }
                let def = hir::HirStructDef {
                    name: s.name.clone(),
                    fields: lowered_fields,
                    is_public: s.is_public,
                    defined_in: ctx2.path.clone(),
                    span: item.span,
                    context_id: None,
                };
                let path = vec![s.name.clone()];
                self.type_definitions
                    .insert(path.clone(), hir::Item::Struct(def));
                self.type_definition_meta
                    .insert(path, (ctx2.path.clone(), s.is_public));
            }
            OwnedItem::Effect(eff) => {
                let ctx2 = ItemContext {
                    span: item.span,
                    path: file.clone(),
                };
                let mut ops: Vec<hir::HirFunctionSignature> = Vec::new();
                for op in &eff.operations {
                    let mut params: Vec<hir::HirParam> = Vec::new();
                    for p in &op.params {
                        if let Ok(t) = self.resolve_type(p, ctx2.clone()) {
                            params.push(hir::HirParam {
                                name: "_".to_string(),
                                ty: t,
                                is_mut: false,
                                span: None,
                            });
                        }
                    }
                    let ret_ty = self
                        .resolve_type(&op.ret_type, ctx2.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    ops.push(hir::HirFunctionSignature {
                        name: op.name.clone(),
                        params,
                        ret_type: ret_ty,
                        effects: vec![],
                    });
                }
                let def = hir::HirEffectDef {
                    name: eff.name.clone(),
                    operations: ops,
                    is_public: eff.is_public,
                    defined_in: ctx2.path.clone(),
                    span: item.span,
                };
                let path = vec![eff.name.clone()];
                self.type_definitions
                    .insert(path.clone(), hir::Item::Effect(def));
                self.type_definition_meta
                    .insert(path, (ctx2.path.clone(), eff.is_public));
            }
            OwnedItem::Handler(h) => {
                let ctx2 = ItemContext {
                    span: item.span,
                    path: file.clone(),
                };
                self.register_handler_value(&h.name, &h.effects, ctx2);
            }
            OwnedItem::Stmt(_) => {
                self.register_handler_alias_stmt(
                    item,
                    ItemContext {
                        span: item.span,
                        path: file.clone(),
                    },
                );
            }
            _ => {
                // Fallback for other item kinds if needed in the future
                let owned = OwnedItemWithSpan {
                    item: item.item.clone(),
                    span: item.span,
                };
                self.register_top_level_item(&owned);
            }
        }
    }

    pub(crate) fn register_builtin_functions(&mut self) {
        let len_sig = hir::HirFunctionSignature {
            name: "len".to_string(),
            params: vec![hir::HirParam {
                name: "s".to_string(),
                ty: hir::Ty::Primitive(hir::PrimitiveTy::Str),
                is_mut: false,
                span: None,
            }],
            ret_type: hir::Ty::Primitive(hir::PrimitiveTy::I32),
            effects: vec![],
        };
        self.add_symbol_to_current_scope(
            "len".to_string(),
            Symbol::Function {
                signature: len_sig,
                is_public: true,
                defined_in: PathBuf::from("<builtin>"),
                decl_span: None,
            },
        );

        // Note: We no longer hard-code fmt::println/io::println built-ins.
        // Standard library functions should be provided by real modules when imports resolve modules.
    }
}
