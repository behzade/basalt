use crate::ast_owned::*; // Your Owned AST definitions
use crate::hir; // Your HIR definitions
use crate::typechecker::errors::{ItemContext, TypeError};
// removed unused imports
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::typechecker::symbols::Symbol;
use crate::hir::{HirContext, HirContextKind, HirSymbolDecl, HirSymbolKind, ContextId};

#[derive(Default)]
pub struct Typechecker {
    /// A stack of scopes. The last element is the current, innermost scope.
    /// Used for resolving local variables and function parameters.
    pub(crate) scopes: Vec<HashMap<String, Symbol>>,

    /// A global map of all defined types (structs, enums, effects).
    /// The key is the canonical, fully-qualified path to the type.
    pub(crate) type_definitions: HashMap<hir::OwnedPath, hir::Item>,

    /// A place to collect errors found during checking.
    pub(crate) errors: Vec<TypeError>,

    /// Context for the current function being checked, needed for 'return' statements.
    pub(crate) current_fn_return_type: Option<hir::Ty>,

    /// Cached map from enum (union) name to its variants for quick lookup.
    /// Populated from type aliases that define unions.
    pub(crate) union_variants: HashMap<hir::OwnedPath, Vec<(String, Option<Vec<hir::Ty>>)>>,

    /// Map of file path -> set of imported names (aliases or last path segment).
    /// Used to suppress spurious unknown-name errors for module identifiers like `io`.
    pub(crate) imports_by_file: HashMap<PathBuf, HashSet<String>>,
    /// Map of file path -> alias name -> fully-qualified module path (e.g., "fmt" -> ["std","fmt"])
    pub(crate) import_alias_map: HashMap<PathBuf, HashMap<String, hir::OwnedPath>>,

    /// Map of type path -> (defined file, is_public)
    pub(crate) type_definition_meta: HashMap<hir::OwnedPath, (PathBuf, bool)>,

    /// Top-level value functions by simple name. Ambiguities are not handled here yet.
    pub(crate) top_level_functions: HashMap<String, (hir::HirFunctionSignature, bool, PathBuf)>,

    /// Inherent and trait methods registered via impl blocks, keyed by nominal type path.
    /// For now we only support inherent methods on nominal types without generics.
    pub(crate) impl_methods: HashMap<hir::OwnedPath, HashMap<String, (hir::HirFunctionSignature, bool, PathBuf, crate::token::SimpleSpan)>>,

    /// Persistent HIR contexts being built during lowering
    pub contexts: Vec<HirContext>,
    /// Stack of active context ids during lowering (function, block, etc.)
    pub(crate) current_context_stack: Vec<ContextId>,
}

// ItemContext is re-exported from errors.rs

impl Typechecker {
    
    // format_ty moved to errors.rs (impl on Typechecker)
    fn new_context(&mut self, kind: HirContextKind, path: &PathBuf, span: crate::token::SimpleSpan) -> ContextId {
        let id = self.contexts.len();
        let ctx = HirContext { id, parent: None, kind, defined_in: path.clone(), span, symbols: Vec::new(), children: Vec::new() };
        self.contexts.push(ctx);
        id
    }

    fn add_child_context(&mut self, parent: ContextId, child: ContextId) {
        if let Some(p) = self.contexts.get_mut(parent) { p.children.push(child); }
        if let Some(c) = self.contexts.get_mut(child) { c.parent = Some(parent); }
    }

    fn set_context_kind(&mut self, id: ContextId, kind: HirContextKind) {
        if let Some(c) = self.contexts.get_mut(id) { c.kind = kind; }
    }

    pub(crate) fn add_symbol_to_context(&mut self, id: ContextId, sym: HirSymbolDecl) {
        if let Some(c) = self.contexts.get_mut(id) { c.symbols.push(sym); }
    }

    pub(crate) fn push_context(&mut self, id: ContextId) { self.current_context_stack.push(id); }
    pub(crate) fn pop_context(&mut self) { let _ = self.current_context_stack.pop(); }
    pub(crate) fn current_context(&self) -> Option<ContextId> { self.current_context_stack.last().copied() }
    pub fn check_program(
        &mut self,
        files: HashMap<PathBuf, Vec<OwnedItemWithSpan>>,
    ) -> Result<Vec<hir::Item>, Vec<TypeError>> {
        // Ensure there is a global scope
        self.enter_scope();

        // Register builtin functions needed by tests
        self.register_builtin_functions();

        // Collect import aliases/names per file for module-aware name handling
        let mut imports_map: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        let mut alias_map: HashMap<PathBuf, HashMap<String, hir::OwnedPath>> = HashMap::new();
        for (path, items) in &files {
            let entry = imports_map.entry(path.clone()).or_default();
            let alias_entry = alias_map.entry(path.clone()).or_default();
            for it in items {
                if let OwnedItem::ImportBlock { imports } = &it.item {
                    for imp in imports {
                        let name = imp
                            .alias
                            .clone()
                            .unwrap_or_else(|| imp.path.last().cloned().unwrap_or_default());
                        if !name.is_empty() {
                            entry.insert(name.clone());
                            alias_entry.insert(name, imp.path.clone());
                        }
                    }
                }
            }
        }
        self.imports_by_file = imports_map;
        self.import_alias_map = alias_map;

        // --- PASS 1: Register all top-level definitions ---
        for (path, items) in &files {
            for item in items {
                self.register_top_level_item_with_file(item, path);
            }
        }

        let mut hir_items: Vec<hir::Item> = Vec::new();
        self.contexts.clear();
        for (path, items) in &files {
            for item in items {
                if let Ok(it) = self.lower_item(item.clone(), path.clone()) {
                    // Capture impl methods for method lookup later
                    if let hir::Item::Impl(impl_block) = &it {
                        if let hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) | hir::Ty::Adt(hir::AdtTy::Enum { name, .. }) = &impl_block.target_type {
                            let mut to_insert: Vec<(String, (hir::HirFunctionSignature, bool, PathBuf, crate::token::SimpleSpan), String)> = Vec::new();
                            for f in &impl_block.methods {
                                let method_name = f.signature.name.clone();
                                let sig = f.signature.clone();
                                let mangled = Self::mangle_method_name(path, name, &method_name);
                                to_insert.push((method_name, (sig, f.is_public, path.clone(), f.span), mangled));
                            }
                            // Insert impl methods
                            {
                                let entry = self.impl_methods.entry(name.clone()).or_default();
                                for (method_name, (sig, is_public, def_path, span), _) in &to_insert {
                                    entry.insert(method_name.clone(), (sig.clone(), *is_public, def_path.clone(), *span));
                                }
                            }
                            // Do not register mangled method symbols in global scope; method
                            // resolution is internal via `impl_methods` to avoid UFCS pollution.
                        }
                    }
                    hir_items.push(it);
                }
            }
        }

        if self.errors.is_empty() {
            Ok(hir_items)
        } else {
            Err(self.errors.clone())
        }
    }

    //================================================================================//
    //                             Lowering Logic
    //================================================================================//

    /// Lowers a single `OwnedItem` from AST to HIR. This is the dispatcher.
    fn lower_item(&mut self, item: OwnedItemWithSpan, path: PathBuf) -> Result<hir::Item, ()> {
        match item.item {
            OwnedItem::Fn(func) => self
                .lower_function(
                    func,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(hir::Item::Fn),
            OwnedItem::Struct(s) => self
                .lower_struct(
                    s,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(hir::Item::Struct),
            OwnedItem::TypeAlias(ta) => self
                .lower_type_alias(
                    ta,
                    ItemContext { span: item.span, path },
                )
                .map(hir::Item::TypeAlias),
            OwnedItem::Enum(e) => self
                .lower_enum(
                    e,
                    ItemContext { span: item.span, path },
                )
                .map(hir::Item::Enum),
            OwnedItem::Effect(eff) => self
                .lower_effect(
                    eff,
                    ItemContext { span: item.span, path },
                )
                .map(hir::Item::Effect),
            OwnedItem::Trait(tr) => self
                .lower_trait(
                    tr,
                    ItemContext { span: item.span, path },
                )
                .map(hir::Item::Trait),
            OwnedItem::Impl(imp) => self
                .lower_impl(
                    imp,
                    ItemContext { span: item.span, path },
                )
                .map(hir::Item::Impl),
            OwnedItem::Handler(h) => self
                .lower_handler(
                    h,
                    ItemContext { span: item.span, path },
                )
                .map(hir::Item::Handler),
            // Skip traits/impls/effects/handlers/methods for this milestone
            _ => Err(()),
        }
    }

    /// Lowers an `OwnedFunction` to a `hir::HirFunction`.
    pub(crate) fn lower_function(
        &mut self,
        func: OwnedFunction,
        context: ItemContext,
    ) -> Result<hir::HirFunction, ()> {
        // 1. Resolve types for the function signature.
        let params: Vec<hir::HirParam> = func
            .params
            .iter()
            .map(|(name, ty)| {
                let resolved_ty = self.resolve_type(ty, context.clone())?;
                Ok(hir::HirParam { name: name.clone().unwrap_or_else(|| "_".to_string()), ty: resolved_ty, span: None })
            })
            .collect::<Result<Vec<_>, ()>>()?;

        let ret_type = match &func.ret_type {
            Some(rt) => self.resolve_type(rt, context.clone())?,
            None => hir::Ty::Special(hir::SpecialTy::Unit), // Default return type is unit
        };

        // TODO: Resolve effect types.
        let effects = Vec::new();

        let signature = hir::HirFunctionSignature {
            name: func.name.clone(),
            params: params.clone(),
            ret_type: ret_type.clone(),
            effects,
        };

        // 2. Set context and scope for checking the body.
        let old_return_type = self.current_fn_return_type.replace(ret_type);
        self.enter_scope();

        // Add function parameters as variables to the new scope.
        for p in &params {
            let symbol = Symbol::Variable {
                ty: p.ty.clone(),
                is_mut: false,
                initialized: true,
                decl_span: None,
            };
            self.add_symbol_to_current_scope(p.name.clone(), symbol);
        }

        // 3. Lower the function body (propagate expected return type to help record literals, etc.).
        // The AST has an `OwnedExpr` body, while the HIR expects a `HirBlock`.
        // We lower the expression with the expected return type and ensure it's a block.
        let body_expr = self.lower_expr_with_expected(
            func.body,
            signature.ret_type.clone(),
            context.clone(),
        )?;
        let body_span = body_expr.span;
        let body_block = match body_expr.kind {
            hir::ExprKind::Block(block) => {
                if block.ty != signature.ret_type {
                    self.errors.push(TypeError {
                        message: format!(
                            "Mismatched return type for function '{}': expected {} but found {}",
                            func.name,
                            Typechecker::format_ty(&signature.ret_type),
                            Typechecker::format_ty(&block.ty)
                        ),
                        context: context.clone(),
                    });
                }
                block
            }
            other_kind => {
                // Allow expression bodies: wrap into a block with this expression as the value
                let ty = body_expr.ty.clone();
                if ty != signature.ret_type {
                    self.errors.push(TypeError {
                        message: format!(
                            "Mismatched return type for function '{}': expected {} but found {}",
                            func.name,
                            Typechecker::format_ty(&signature.ret_type),
                            Typechecker::format_ty(&ty)
                        ),
                        context: context.clone(),
                    });
                }
                hir::HirBlock {
                    stmts: vec![],
                    last_expr: Some(Box::new(hir::Expr { kind: other_kind, ty: ty.clone(), span: context.span, resolution: None })),
                    ty,
                }
            }
        };

        // 4. Clean up scope and context.
        self.leave_scope();
        self.current_fn_return_type = old_return_type;

        // Build a function context and capture params/lets
        let ctx_id = self.new_context(HirContextKind::Function, &context.path, context.span);
        self.push_context(ctx_id);
        // record params in context symbols
        for p in &params {
            self.add_symbol_to_context(ctx_id, HirSymbolDecl {
                name: p.name.clone(),
                kind: HirSymbolKind::Param,
                ty: Some(p.ty.clone()),
                is_mut: Some(false),
                span: body_span,
                name_span: p.span,
            });
        }

        let result = hir::HirFunction {
            signature,
            body: body_block,
            is_public: func.is_public,
            defined_in: context.path.clone(),
            // Use the body expression span as a best-effort function span for navigation
            span: body_span,
            context_id: Some(ctx_id),
        };
        self.pop_context();
        Ok(result)
    }

    /// Lowers a `OwnedStructDef` to a `hir::HirStructDef`.
    /// This is simpler than a function as it has no executable body.
    fn lower_struct(
        &mut self,
        s: OwnedStructDef,
        context: ItemContext,
    ) -> Result<hir::HirStructDef, ()> {
        let fields = s
            .fields
            .into_iter()
            .map(|(name, ty)| self.resolve_type(&ty, context.clone()).map(|t| hir::HirField { name, ty: t, name_span: None }))
            .collect::<Result<Vec<_>, _>>()?;

        let ctx_id = self.new_context(HirContextKind::Struct, &context.path, context.span);
        // Record fields as symbols in the struct context
        for f in &fields {
            self.add_symbol_to_context(ctx_id, HirSymbolDecl {
                name: f.name.clone(),
                kind: HirSymbolKind::Field,
                ty: Some(f.ty.clone()),
                is_mut: Some(false),
                span: context.span,
                name_span: f.name_span,
            });
        }
        Ok(hir::HirStructDef {
            name: s.name,
            fields,
            is_public: s.is_public,
            defined_in: context.path.clone(),
            span: context.span,
            context_id: Some(ctx_id),
        })
    }

    fn lower_enum(
        &mut self,
        e: OwnedEnumDef,
        context: ItemContext,
    ) -> Result<hir::HirEnumDef, ()> {
        let name = e.name.unwrap_or_default();
        let mut variants: Vec<hir::HirEnumVariant> = Vec::new();
        for (vname, payload_opt) in e.variants {
            let lowered_payload = match payload_opt {
                Some(ts) => {
                    let mut out: Vec<hir::Ty> = Vec::new();
                    for t in ts {
                        out.push(self.resolve_type(&t, context.clone())?);
                    }
                    Some(out)
                }
                None => None,
            };
            variants.push(hir::HirEnumVariant { name: vname, payload: lowered_payload, name_span: None });
        }
        let ctx_id = self.new_context(HirContextKind::Enum, &context.path, context.span);
        for v in &variants {
            self.add_symbol_to_context(ctx_id, HirSymbolDecl {
                name: v.name.clone(),
                kind: HirSymbolKind::EnumVariant,
                ty: None,
                is_mut: None,
                span: context.span,
                name_span: v.name_span,
            });
        }
        Ok(hir::HirEnumDef { name, variants, is_public: e.is_public, defined_in: context.path.clone(), span: context.span, context_id: Some(ctx_id) })
    }

    fn lower_type_alias(
        &mut self,
        ta: OwnedTypeAliasDef,
        context: ItemContext,
    ) -> Result<hir::HirTypeAlias, ()> {
        use crate::ast_owned::OwnedTypeAliasBody as Body;
        let name = ta.name.clone();
        match ta.aliased {
            Body::Type(t) => {
                let aliased = self.resolve_type(&t, context.clone())?;
                Ok(hir::HirTypeAlias { name, aliased, is_public: ta.is_public, defined_in: context.path.clone(), span: context.span })
            }
            Body::Union(variants) => {
                // Lower to enum def and alias to that nominal enum
                let mut lowered: Vec<hir::HirEnumVariant> = Vec::new();
                for (vname, payload_ty) in variants {
                    let payload_tys = Some(vec![self.resolve_type(&payload_ty, context.clone())?]);
                    lowered.push(hir::HirEnumVariant { name: vname, payload: payload_tys, name_span: None });
                }
                let def = hir::HirEnumDef { name: ta.name.clone(), variants: lowered.clone(), is_public: ta.is_public, defined_in: context.path.clone(), span: context.span, context_id: None };
                let path = vec![ta.name.clone()];
                self.type_definitions.insert(path.clone(), hir::Item::Enum(def));
                // Keep internal union_variants as tuple vec
                let uv: Vec<(String, Option<Vec<hir::Ty>>)> = lowered
                    .iter()
                    .map(|v| (v.name.clone(), v.payload.clone()))
                    .collect();
                self.union_variants.insert(path.clone(), uv);
                let aliased = hir::Ty::Adt(hir::AdtTy::Enum { name: path, generics: vec![] });
                Ok(hir::HirTypeAlias { name: ta.name, aliased, is_public: true, defined_in: context.path.clone(), span: context.span })
            }
        }
    }

    fn lower_effect(
        &mut self,
        eff: OwnedEffectDef,
        context: ItemContext,
    ) -> Result<hir::HirEffectDef, ()> {
        let mut operations: Vec<hir::HirFunctionSignature> = Vec::new();
        for op in &eff.operations {
            let mut params: Vec<hir::HirParam> = Vec::new();
            for p in &op.params {
                params.push(hir::HirParam { name: "_".to_string(), ty: self.resolve_type(p, context.clone())?, span: None });
            }
            let ret_type = self.resolve_type(&op.ret_type, context.clone())?;
            operations.push(hir::HirFunctionSignature { name: op.name.clone(), params, ret_type, effects: vec![] });
        }
        Ok(hir::HirEffectDef { name: eff.name, operations, is_public: eff.is_public, defined_in: context.path.clone(), span: context.span })
    }

    fn lower_trait(
        &mut self,
        tr: OwnedTraitDef,
        context: ItemContext,
    ) -> Result<hir::HirTraitDef, ()> {
        let mut methods: Vec<hir::HirFunctionSignature> = Vec::new();
        for m in &tr.methods {
            let mut params: Vec<hir::HirParam> = Vec::new();
            for (name_opt, ty) in &m.params {
                let pname = name_opt.clone().unwrap_or("_".to_string());
                let pty = self.resolve_type(ty, context.clone())?;
                params.push(hir::HirParam { name: pname, ty: pty, span: None });
            }
            let ret_type = match &m.ret_type {
                Some(rt) => self.resolve_type(rt, context.clone())?,
                None => hir::Ty::Special(hir::SpecialTy::Unit),
            };
            methods.push(hir::HirFunctionSignature { name: m.name.clone(), params, ret_type, effects: vec![] });
        }
        Ok(hir::HirTraitDef { name: tr.name, methods, is_public: tr.is_public, defined_in: context.path.clone(), span: context.span })
    }

    fn lower_impl(
        &mut self,
        imp: OwnedImplBlock,
        context: ItemContext,
    ) -> Result<hir::HirImplBlock, ()> {
        let trait_path = imp.interface.clone();
        let target_type_owned = imp.target_type.clone();
        let target_type = self.resolve_type(&imp.target_type, context.clone())?;
        let mut methods = Vec::new();
        for mut f in imp.methods {
            // If first param is self-like (named same as identifier) without explicit type, coerce to impl target
            if let Some((name_opt, pty)) = f.params.get_mut(0) {
                if let Some(pn) = name_opt {
                    if pty.path.len() == 1 && pty.path[0] == *pn {
                        *pty = target_type_owned.clone();
                    }
                }
            }
            let mut hf = self.lower_function(f, context.clone())?;
            // Mark the function context as ImplMethod for better classification
            if let Some(cid) = hf.context_id {
                self.set_context_kind(cid, HirContextKind::ImplMethod);
            }
            methods.push(hf);
        }
        Ok(hir::HirImplBlock { trait_path, target_type, methods, defined_in: context.path.clone(), span: context.span })
    }

    fn lower_handler(
        &mut self,
        h: OwnedHandlerDef,
        context: ItemContext,
    ) -> Result<hir::HirHandlerDef, ()> {
        // Convert effect names to canonical types when possible; unknowns become Generic placeholders
        let mut effects: Vec<hir::Ty> = Vec::new();
        for eff_name in &h.effects {
            let path = vec![eff_name.clone()];
            if let Some(hir::Item::Effect(_)) = self.type_definitions.get(&path) {
                effects.push(hir::Ty::Adt(hir::AdtTy::Effect { name: path, generics: vec![] }));
            } else {
                effects.push(hir::Ty::Generic(eff_name.clone()));
            }
        }
        let mut functions = Vec::new();
        for f in h.functions {
            functions.push(self.lower_function(f, context.clone())?);
        }
        Ok(hir::HirHandlerDef { name: h.name, effects, functions, is_public: h.is_public, defined_in: context.path.clone(), span: context.span })
    }

    //================================================================================//
    //                             Helper & Utility Functions
    //================================================================================//

    /// **Crucial**: Resolves an AST type representation into a canonical HIR type.
    pub(crate) fn resolve_type(&mut self, owned_ty: &OwnedType, context: ItemContext) -> Result<hir::Ty, ()> {
        // This is a placeholder for a complex process. A real implementation must:
        // 1. Handle primitive types ("i32", "bool", etc.).
        // 2. Look up custom types (structs, enums) in `type_definitions`.
        // 3. Resolve generic type parameters.
        // 4. Handle fully-qualified paths.
        let type_name = owned_ty.path.join("::");
        match type_name.as_str() {
            "i32" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I32)),
            "i64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I64)),
            "f64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::F64)),
            "bool" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Bool)),
            "str" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Str)),
            "()" => Ok(hir::Ty::Special(hir::SpecialTy::Unit)),
            _ => {
                // Try to resolve against registered type definitions
                let path_vec = owned_ty.path.clone();
                if let Some(item) = self.type_definitions.get(&path_vec) {
                    // Enforce visibility at module boundaries for types
                    if let Some((defined_in, is_public)) = self.type_definition_meta.get(&path_vec) {
                        if !*is_public && defined_in != &context.path {
                            self.errors.push(TypeError {
                                message: format!("Type `{}` is private", type_name),
                                context: context.clone(),
                            });
                            return Err(());
                        }
                    }
                    match item {
                        hir::Item::Struct(_) => Ok(hir::Ty::Adt(hir::AdtTy::Struct {
                            name: path_vec,
                            generics: vec![],
                        })),
                        hir::Item::Enum(_) => Ok(hir::Ty::Adt(hir::AdtTy::Enum {
                            name: path_vec,
                            generics: vec![],
                        })),
                        hir::Item::Effect(_) => Ok(hir::Ty::Adt(hir::AdtTy::Effect {
                            name: path_vec,
                            generics: vec![],
                        })),
                        _ => {
                            self.errors.push(TypeError {
                                message: format!("Unsupported type item for {}", type_name),
                                context: context.clone(),
                            });
                            Err(())
                        }
                    }
                } else if owned_ty.path.len() == 1 {
                    // Treat single-segment unknown types as generics, e.g., T
                    Ok(hir::Ty::Generic(owned_ty.path[0].clone()))
                } else {
                self.errors.push(TypeError {
                    message: format!("Unknown type: {}", type_name),
                    context: context.clone(),
                });
                Err(())
                }
            }
        }
    }

}
