use crate::ast_owned::*; // Your Owned AST definitions
use crate::hir; // Your HIR definitions
use crate::hir_validation;
use crate::type_unifier::TypeUnifier;
use crate::typechecker::errors::{ItemContext, TypeError};
// removed unused imports
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::hir::{ContextId, HirContext, HirContextKind, HirSymbolDecl, HirSymbolKind};
use crate::typechecker::resolver::{ResolvedNominalType, TypeResolveError};
use crate::typechecker::symbols::Symbol;

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

    /// Generic parameters declared on type aliases, keyed by alias path.
    pub(crate) type_alias_generics: HashMap<hir::OwnedPath, Vec<String>>,

    /// Map of file path -> set of imported names (aliases or last path segment).
    /// Used to suppress spurious unknown-name errors for module identifiers like `io`.
    pub(crate) imports_by_file: HashMap<PathBuf, HashSet<String>>,
    /// Map of file path -> alias name -> fully-qualified module path (e.g., "fmt" -> ["std","fmt"])
    pub(crate) import_alias_map: HashMap<PathBuf, HashMap<String, hir::OwnedPath>>,

    /// Map of type path -> (defined file, is_public)
    pub(crate) type_definition_meta: HashMap<hir::OwnedPath, (PathBuf, bool)>,

    /// Top-level value functions by simple name. Ambiguities are not handled here yet.
    pub(crate) top_level_functions: HashMap<String, (hir::HirFunctionSignature, bool, PathBuf)>,

    /// Top-level handler values by simple name. The vector is the effects this
    /// handler value can discharge after any static handler composition.
    pub(crate) handler_values: HashMap<String, Vec<hir::Ty>>,

    /// Top-level named memory regions available for explicit allocation placement.
    pub(crate) memory_regions: HashSet<String>,

    /// Simple top-level handler aliases of the form `let X = Base with {A, B}`.
    pub(crate) handler_aliases: HashMap<String, (String, Vec<String>)>,

    /// Persistent HIR contexts being built during lowering
    pub contexts: Vec<HirContext>,
    /// Stack of active context ids during lowering (function, block, etc.)
    pub(crate) current_context_stack: Vec<ContextId>,
    /// Stack of currently allowed effects during lowering (top is current function)
    pub(crate) current_effects_stack: Vec<Vec<hir::Ty>>,
    pub(crate) unsafe_depth: usize,
}

// ItemContext is re-exported from errors.rs

impl Typechecker {
    // format_ty moved to errors.rs (impl on Typechecker)
    fn new_context(
        &mut self,
        kind: HirContextKind,
        path: &PathBuf,
        span: crate::token::SimpleSpan,
    ) -> ContextId {
        let id = self.contexts.len();
        let ctx = HirContext {
            id,
            parent: None,
            kind,
            defined_in: path.clone(),
            span,
            symbols: Vec::new(),
            children: Vec::new(),
        };
        self.contexts.push(ctx);
        id
    }

    pub(crate) fn add_symbol_to_context(&mut self, id: ContextId, sym: HirSymbolDecl) {
        if let Some(c) = self.contexts.get_mut(id) {
            c.symbols.push(sym);
        }
    }

    pub(crate) fn push_context(&mut self, id: ContextId) {
        self.current_context_stack.push(id);
    }
    pub(crate) fn pop_context(&mut self) {
        let _ = self.current_context_stack.pop();
    }
    pub(crate) fn current_context(&self) -> Option<ContextId> {
        self.current_context_stack.last().copied()
    }

    pub(crate) fn is_runtime_file(&self, path: &PathBuf) -> bool {
        let s = path.to_string_lossy();
        s.contains("/modules/std/runtime/") || s.contains("\\modules\\std\\runtime\\")
    }

    pub(crate) fn is_raw_runtime_intrinsic(&self, path: &PathBuf, name: &str) -> bool {
        self.is_runtime_file(path) && matches!(name, "alloc" | "free")
    }
    pub fn check_program(
        &mut self,
        files: HashMap<PathBuf, Vec<OwnedItemWithSpan>>,
    ) -> Result<Vec<hir::Item>, Vec<TypeError>> {
        // Ensure there is a global scope
        self.enter_scope();

        // Register builtin functions needed by tests
        self.register_builtin_functions();
        let mut file_entries: Vec<(PathBuf, Vec<OwnedItemWithSpan>)> = files.into_iter().collect();
        file_entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        // Collect import aliases/names per file for module-aware name handling
        let mut imports_map: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        let mut alias_map: HashMap<PathBuf, HashMap<String, hir::OwnedPath>> = HashMap::new();
        for (path, items) in &file_entries {
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
        for (path, items) in &file_entries {
            for item in items {
                self.register_top_level_item_with_file(item, path);
            }
        }

        let mut hir_items: Vec<hir::Item> = Vec::new();
        self.contexts.clear();
        for (path, items) in &file_entries {
            for item in items {
                if let Ok(items) = self.lower_item(item.clone(), path.clone()) {
                    hir_items.extend(items);
                }
            }
        }

        if self.errors.is_empty() {
            if let Err(validation_errors) = hir_validation::validate_program(&hir_items) {
                for error in validation_errors {
                    self.errors.push(TypeError {
                        message: format!("HIR validation failed: {}", error.message),
                        context: ItemContext {
                            span: error.span,
                            path: error.path,
                        },
                    });
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
    fn lower_item(&mut self, item: OwnedItemWithSpan, path: PathBuf) -> Result<Vec<hir::Item>, ()> {
        match item.item {
            OwnedItem::Fn(func) => self
                .lower_function(
                    func,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(|item| vec![hir::Item::Fn(item)]),
            OwnedItem::Memory(memory) => {
                if memory.byte_limit == 0 {
                    self.errors.push(TypeError {
                        message: format!(
                            "Memory region '{}' must have a non-zero byte limit",
                            memory.name
                        ),
                        context: ItemContext {
                            span: item.span,
                            path: path.clone(),
                        },
                    });
                    return Err(());
                }
                Ok(vec![hir::Item::Memory(hir::HirMemoryDef {
                    name: memory.name,
                    byte_limit: memory.byte_limit,
                    object_limit: memory.object_limit,
                    defined_in: path,
                    span: item.span,
                })])
            }
            OwnedItem::Struct(s) => self
                .lower_struct(
                    s,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(|item| vec![hir::Item::Struct(item)]),
            OwnedItem::TypeAlias(ta) => self
                .lower_type_alias(
                    ta,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(|(enum_def, alias)| {
                    let mut items = Vec::new();
                    if let Some(enum_def) = enum_def {
                        items.push(hir::Item::Enum(enum_def));
                    }
                    items.push(hir::Item::TypeAlias(alias));
                    items
                }),
            OwnedItem::Enum(e) => self
                .lower_enum(
                    e,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(|item| vec![hir::Item::Enum(item)]),
            OwnedItem::Effect(eff) => self
                .lower_effect(
                    eff,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(|item| vec![hir::Item::Effect(item)]),
            OwnedItem::Handler(h) => self
                .lower_handler(
                    h,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(|item| vec![hir::Item::Handler(item)]),
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
            .map(|(is_mut, name, ty)| {
                let resolved_ty = self.resolve_type(ty, context.clone())?;
                Ok(hir::HirParam {
                    name: name.clone().unwrap_or_else(|| "_".to_string()),
                    ty: resolved_ty,
                    is_mut: *is_mut,
                    span: None,
                })
            })
            .collect::<Result<Vec<_>, ()>>()?;

        let ret_type = match &func.ret_type {
            Some(rt) => self.resolve_type(rt, context.clone())?,
            None => hir::Ty::Special(hir::SpecialTy::Unit), // Default return type is unit
        };

        // Resolve effect types from the function signature's effect list
        let mut effects_vec: Vec<hir::Ty> = Vec::new();
        for eff_name in &func.effects {
            if let Some(effect) = self.resolve_effect_type_name(eff_name) {
                effects_vec.push(effect);
            } else {
                self.errors.push(TypeError {
                    message: format!("Unknown effect `{}`", eff_name),
                    context: context.clone(),
                });
            }
        }

        let signature = hir::HirFunctionSignature {
            name: func.name.clone(),
            params: params.clone(),
            ret_type: ret_type.clone(),
            effects: effects_vec.clone(),
        };

        // 2. Set context and scope for checking the body.
        let old_return_type = self.current_fn_return_type.replace(ret_type);
        // Push allowed effects for this function while lowering its body
        self.current_effects_stack.push(effects_vec.clone());
        self.enter_scope();

        // Add function parameters as variables to the new scope.
        for p in &params {
            let symbol = Symbol::Variable {
                ty: p.ty.clone(),
                is_mut: p.is_mut,
                initialized: true,
                decl_span: None,
            };
            self.add_symbol_to_current_scope(p.name.clone(), symbol);
        }

        let extern_kind = if func.is_extern {
            Some(if self.is_runtime_file(&context.path) {
                hir::HirExternKind::RuntimeIntrinsic
            } else {
                hir::HirExternKind::Foreign
            })
        } else {
            None
        };

        if !func.is_extern && self.is_runtime_file(&context.path) {
            self.errors.push(TypeError {
                message: "Functions in std::runtime must be extern declarations".to_string(),
                context: context.clone(),
            });
            return Err(());
        }

        // 3. Lower the function body. Extern declarations have no Basalt body;
        // their implementation lives in the interpreter/native runtime.
        let (body_block, body_span) = if func.is_extern {
            let block = hir::HirBlock {
                stmts: vec![],
                last_expr: None,
                ty: signature.ret_type.clone(),
            };
            (block, context.span)
        } else {
            let body_expr = self.lower_expr_with_expected(
                func.body,
                signature.ret_type.clone(),
                context.clone(),
            )?;
            let body_span = body_expr.span;
            let body_block = match body_expr.kind {
                hir::ExprKind::Block(mut block) => {
                    if block_always_returns(&block) {
                        block.ty = hir::Ty::Special(hir::SpecialTy::Never);
                    }
                    if !block_always_returns(&block)
                        && !TypeUnifier::is_assignable(&block.ty, &signature.ret_type)
                    {
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
                    let ty = body_expr.ty.clone();
                    if !TypeUnifier::is_assignable(&ty, &signature.ret_type) {
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
                        last_expr: Some(Box::new(hir::Expr {
                            kind: other_kind,
                            ty: ty.clone(),
                            span: context.span,
                            resolution: None,
                        })),
                        ty,
                    }
                }
            };
            (body_block, body_span)
        };

        // 4. Clean up scope and context.
        self.leave_scope();
        self.current_fn_return_type = old_return_type;
        let _ = self.current_effects_stack.pop();

        // Build a function context and capture params/lets
        let ctx_id = self.new_context(HirContextKind::Function, &context.path, context.span);
        self.push_context(ctx_id);
        // record params in context symbols
        for p in &params {
            self.add_symbol_to_context(
                ctx_id,
                HirSymbolDecl {
                    name: p.name.clone(),
                    kind: HirSymbolKind::Param,
                    ty: Some(p.ty.clone()),
                    is_mut: Some(p.is_mut),
                    span: body_span,
                    name_span: p.span,
                },
            );
        }

        let result = hir::HirFunction {
            signature,
            body: body_block,
            is_public: func.is_public,
            extern_kind,
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
            .map(|(name, ty)| {
                self.resolve_type(&ty, context.clone())
                    .map(|t| hir::HirField {
                        name,
                        ty: t,
                        name_span: None,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let ctx_id = self.new_context(HirContextKind::Struct, &context.path, context.span);
        // Record fields as symbols in the struct context
        for f in &fields {
            self.add_symbol_to_context(
                ctx_id,
                HirSymbolDecl {
                    name: f.name.clone(),
                    kind: HirSymbolKind::Field,
                    ty: Some(f.ty.clone()),
                    is_mut: Some(false),
                    span: context.span,
                    name_span: f.name_span,
                },
            );
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

    fn lower_enum(&mut self, e: OwnedEnumDef, context: ItemContext) -> Result<hir::HirEnumDef, ()> {
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
            variants.push(hir::HirEnumVariant {
                name: vname,
                payload: lowered_payload,
                name_span: None,
            });
        }
        let ctx_id = self.new_context(HirContextKind::Enum, &context.path, context.span);
        for v in &variants {
            self.add_symbol_to_context(
                ctx_id,
                HirSymbolDecl {
                    name: v.name.clone(),
                    kind: HirSymbolKind::EnumVariant,
                    ty: None,
                    is_mut: None,
                    span: context.span,
                    name_span: v.name_span,
                },
            );
        }
        Ok(hir::HirEnumDef {
            name,
            variants,
            is_public: e.is_public,
            defined_in: context.path.clone(),
            span: context.span,
            context_id: Some(ctx_id),
        })
    }

    fn lower_type_alias(
        &mut self,
        ta: OwnedTypeAliasDef,
        context: ItemContext,
    ) -> Result<(Option<hir::HirEnumDef>, hir::HirTypeAlias), ()> {
        use crate::ast_owned::OwnedTypeAliasBody as Body;
        let name = ta.name.clone();
        match ta.aliased {
            Body::Type(t) => {
                let aliased = self.resolve_type(&t, context.clone())?;
                Ok((
                    None,
                    hir::HirTypeAlias {
                        name,
                        aliased,
                        is_public: ta.is_public,
                        defined_in: context.path.clone(),
                        span: context.span,
                    },
                ))
            }
            Body::Union(variants) => {
                // Lower to enum def and alias to that nominal enum
                let mut lowered: Vec<hir::HirEnumVariant> = Vec::new();
                for (vname, payload_ty) in variants {
                    let payload_tys = Some(vec![self.resolve_type(&payload_ty, context.clone())?]);
                    lowered.push(hir::HirEnumVariant {
                        name: vname,
                        payload: payload_tys,
                        name_span: None,
                    });
                }
                let def = hir::HirEnumDef {
                    name: ta.name.clone(),
                    variants: lowered.clone(),
                    is_public: ta.is_public,
                    defined_in: context.path.clone(),
                    span: context.span,
                    context_id: None,
                };
                let path = vec![ta.name.clone()];
                self.type_alias_generics
                    .insert(path.clone(), ta.generics.clone());
                self.type_definitions
                    .insert(path.clone(), hir::Item::Enum(def.clone()));
                // Keep internal union_variants as tuple vec
                let uv: Vec<(String, Option<Vec<hir::Ty>>)> = lowered
                    .iter()
                    .map(|v| (v.name.clone(), v.payload.clone()))
                    .collect();
                self.union_variants.insert(path.clone(), uv);
                let aliased = hir::Ty::Adt(hir::AdtTy::Enum {
                    name: path,
                    generics: vec![],
                });
                Ok((
                    Some(def),
                    hir::HirTypeAlias {
                        name: ta.name,
                        aliased,
                        is_public: true,
                        defined_in: context.path.clone(),
                        span: context.span,
                    },
                ))
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
                params.push(hir::HirParam {
                    name: "_".to_string(),
                    ty: self.resolve_type(p, context.clone())?,
                    is_mut: false,
                    span: None,
                });
            }
            let ret_type = self.resolve_type(&op.ret_type, context.clone())?;
            operations.push(hir::HirFunctionSignature {
                name: op.name.clone(),
                params,
                ret_type,
                effects: vec![],
            });
        }
        Ok(hir::HirEffectDef {
            name: eff.name,
            operations,
            is_public: eff.is_public,
            defined_in: context.path.clone(),
            span: context.span,
        })
    }

    fn lower_handler(
        &mut self,
        h: OwnedHandlerDef,
        context: ItemContext,
    ) -> Result<hir::HirHandlerDef, ()> {
        // Convert effect names to canonical types when possible; unknowns become Generic placeholders
        let mut effects: Vec<hir::Ty> = Vec::new();
        for eff_name in &h.effects {
            if let Some(effect) = self.resolve_effect_type_name(eff_name) {
                effects.push(effect);
            } else {
                self.errors.push(TypeError {
                    message: format!("Unknown effect `{}`", eff_name),
                    context: context.clone(),
                });
            }
        }

        // Handler must implement operations for its primary effect (first in list)
        let mut functions = Vec::new();
        let primary_effect_path: Option<hir::OwnedPath> = effects.iter().find_map(|t| {
            if let hir::Ty::Adt(hir::AdtTy::Effect { name, .. }) = t {
                Some(name.clone())
            } else {
                None
            }
        });
        let all_effect_ops: Vec<hir::HirFunctionSignature> = primary_effect_path
            .as_ref()
            .and_then(|p| self.resolve_effect_operations(p))
            .unwrap_or_default();

        // Lower functions with allowed effects = handler's declared effects
        for f in h.functions {
            // temporarily push allowed effects for handler methods
            self.current_effects_stack.push(effects.clone());
            let lowered = self.lower_function(f.clone(), context.clone())?;
            let _ = self.current_effects_stack.pop();
            functions.push(lowered.clone());
        }

        // Validate signatures: each op of primary effect must be implemented by a function with same name and compatible signature
        if let Some(_p) = &primary_effect_path {
            for op in &all_effect_ops {
                let mut found = false;
                for hf in &functions {
                    if hf.signature.name == op.name {
                        found = true;
                        // Compare params count and return type (simple check)
                        if hf.signature.params.len() != op.params.len()
                            || !TypeUnifier::is_assignable(&hf.signature.ret_type, &op.ret_type)
                        {
                            self.errors.push(TypeError { message: format!(
                                "Handler method `{}` must match effect op signature: expected fn({}) -> {}",
                                op.name,
                                op.params.iter().map(|p| Typechecker::format_ty(&p.ty)).collect::<Vec<_>>().join(", "),
                                Typechecker::format_ty(&op.ret_type)
                            ), context: context.clone() });
                        }
                    }
                }
                if !found {
                    self.errors.push(TypeError {
                        message: format!(
                            "Handler is missing implementation for effect op `{}`",
                            op.name
                        ),
                        context: context.clone(),
                    });
                }
            }
        }

        // Validate that each handler method only declares/uses effects allowed by the handler's effect list.
        let allowed_effect_names: Vec<String> = effects
            .iter()
            .filter_map(|t| match t {
                hir::Ty::Adt(hir::AdtTy::Effect { name, .. }) => name.last().cloned(),
                hir::Ty::Generic(n) => Some(n.clone()),
                _ => None,
            })
            .collect();
        for hf in &functions {
            for e in &hf.signature.effects {
                let ename_opt = match e {
                    hir::Ty::Adt(hir::AdtTy::Effect { name, .. }) => name.last().cloned(),
                    hir::Ty::Generic(n) => Some(n.clone()),
                    _ => None,
                };
                if let Some(ename) = ename_opt {
                    if !allowed_effect_names.iter().any(|n| n == &ename) {
                        self.errors.push(TypeError { message: format!(
                            "Handler method `{}` declares effect `{}` which is not in handler's effect list",
                            hf.signature.name, ename
                        ), context: context.clone() });
                    }
                }
            }
        }

        Ok(hir::HirHandlerDef {
            name: h.name,
            effects,
            functions,
            is_public: h.is_public,
            defined_in: context.path.clone(),
            span: context.span,
        })
    }

    //================================================================================//
    //                             Helper & Utility Functions
    //================================================================================//

    /// **Crucial**: Resolves an AST type representation into a canonical HIR type.
    pub(crate) fn resolve_type(
        &mut self,
        owned_ty: &OwnedType,
        context: ItemContext,
    ) -> Result<hir::Ty, ()> {
        // This is a placeholder for a complex process. A real implementation must:
        // 1. Handle primitive types ("i32", "bool", etc.).
        // 2. Look up custom types (structs, enums) in `type_definitions`.
        // 3. Resolve generic type parameters.
        // 4. Handle fully-qualified paths.
        let type_name = owned_ty.path.join("::");
        match type_name.as_str() {
            "byte" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Byte)),
            "i8" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I8)),
            "i16" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I16)),
            "i32" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I32)),
            "i64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I64)),
            "u8" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::U8)),
            "u16" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::U16)),
            "u32" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::U32)),
            "u64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::U64)),
            "f32" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::F32)),
            "f64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::F64)),
            "bool" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Bool)),
            "str" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Str)),
            // Map built-in unit type name to Unit special type
            "unit" => Ok(hir::Ty::Special(hir::SpecialTy::Unit)),
            "()" => Ok(hir::Ty::Special(hir::SpecialTy::Unit)),
            "!" => Ok(hir::Ty::Special(hir::SpecialTy::Never)),
            // Function type lowering if we have structured metadata
            "fn" => {
                if let (Some(params), Some(ret)) =
                    (owned_ty.fn_params.as_ref(), owned_ty.fn_ret.as_ref())
                {
                    let mut lowered_params: Vec<hir::Ty> = Vec::new();
                    for p in params {
                        lowered_params.push(self.resolve_type(p, context.clone())?);
                    }
                    let lowered_ret = self.resolve_type(ret, context.clone())?;
                    let mut lowered_effects: Vec<hir::Ty> = Vec::new();
                    if let Some(effs) = &owned_ty.fn_effects {
                        for e in effs {
                            lowered_effects.push(self.resolve_type(e, context.clone())?);
                        }
                    }
                    Ok(hir::Ty::Function {
                        param_types: lowered_params,
                        ret_type: Box::new(lowered_ret),
                        effects: lowered_effects,
                    })
                } else {
                    // Fallback: treat as generic if structure is missing
                    Ok(hir::Ty::Generic("fn".to_string()))
                }
            }
            _ => {
                // Try to resolve against registered type definitions
                let path_vec = owned_ty.path.clone();
                match self.resolve_nominal_type_path(&path_vec, &context) {
                    Ok(Some(ResolvedNominalType::Struct)) => {
                        let generics = owned_ty
                            .generics
                            .iter()
                            .map(|g| self.resolve_type(g, context.clone()))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(hir::Ty::Adt(hir::AdtTy::Struct {
                            name: path_vec,
                            generics,
                        }))
                    }
                    Ok(Some(ResolvedNominalType::Enum)) => {
                        let generics = owned_ty
                            .generics
                            .iter()
                            .map(|g| self.resolve_type(g, context.clone()))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(hir::Ty::Adt(hir::AdtTy::Enum {
                            name: path_vec,
                            generics,
                        }))
                    }
                    Ok(Some(ResolvedNominalType::Effect)) => {
                        let generics = owned_ty
                            .generics
                            .iter()
                            .map(|g| self.resolve_type(g, context.clone()))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(hir::Ty::Adt(hir::AdtTy::Effect {
                            name: path_vec,
                            generics,
                        }))
                    }
                    Ok(Some(ResolvedNominalType::Unsupported)) => {
                        self.errors.push(TypeError {
                            message: format!("Unsupported type item for {}", type_name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                    Ok(None) if owned_ty.path.len() == 1 => {
                        // Treat single-segment unknown types as generics, e.g., T
                        Ok(hir::Ty::Generic(owned_ty.path[0].clone()))
                    }
                    Ok(None) => {
                        self.errors.push(TypeError {
                            message: format!("Unknown type: {}", type_name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                    Err(TypeResolveError::PrivateType { name }) => {
                        self.errors.push(TypeError {
                            message: format!("Type `{}` is private", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                }
            }
        }
    }
}

fn block_always_returns(block: &hir::HirBlock) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        hir::Stmt::Return { .. } => true,
        hir::Stmt::Expr { expr, .. } => expr_always_returns(expr),
        _ => false,
    }) || block.last_expr.as_deref().is_some_and(expr_always_returns)
}

fn expr_always_returns(expr: &hir::Expr) -> bool {
    match &expr.kind {
        hir::ExprKind::Block(block) | hir::ExprKind::Handle { body: block, .. } => {
            block_always_returns(block)
        }
        hir::ExprKind::If {
            then_block,
            else_block: Some(else_expr),
            ..
        } => block_always_returns(then_block) && expr_always_returns(else_expr),
        hir::ExprKind::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|(_, arm)| expr_always_returns(arm))
        }
        _ => false,
    }
}
