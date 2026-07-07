use std::collections::HashMap;
use std::path::PathBuf;

use crate::hir;

#[derive(Clone)]
pub struct HirEnumVariantIndexEntry {
    pub enum_path: hir::OwnedPath,
    pub payload: Option<Vec<hir::Ty>>,
}

#[derive(Clone, Default)]
pub struct HirIndex {
    functions: HashMap<String, hir::HirFunction>,
    resolved_functions: HashMap<PathBuf, HashMap<String, hir::HirFunction>>,
    structs: HashMap<hir::OwnedPath, hir::HirStructDef>,
    enums: HashMap<hir::OwnedPath, hir::HirEnumDef>,
    effects: HashMap<hir::OwnedPath, hir::HirEffectDef>,
    handlers: HashMap<String, hir::HirHandlerDef>,
    enum_variants: HashMap<hir::OwnedPath, HirEnumVariantIndexEntry>,
}

impl HirIndex {
    pub fn from_items(items: &[hir::Item]) -> Self {
        let mut index = Self::default();

        for item in items {
            match item {
                hir::Item::Fn(function) => {
                    index
                        .resolved_functions
                        .entry(function.defined_in.clone())
                        .or_default()
                        .insert(function.signature.name.clone(), function.clone());
                    index
                        .functions
                        .insert(function.signature.name.clone(), function.clone());
                }
                hir::Item::Struct(struct_def) => {
                    index
                        .structs
                        .insert(vec![struct_def.name.clone()], struct_def.clone());
                }
                hir::Item::Enum(enum_def) => {
                    let enum_path = vec![enum_def.name.clone()];
                    index.enums.insert(enum_path.clone(), enum_def.clone());
                    for variant in &enum_def.variants {
                        let mut variant_path = enum_path.clone();
                        variant_path.push(variant.name.clone());
                        index.enum_variants.insert(
                            variant_path,
                            HirEnumVariantIndexEntry {
                                enum_path: enum_path.clone(),
                                payload: variant.payload.clone(),
                            },
                        );
                    }
                }
                hir::Item::Effect(effect) => {
                    index
                        .effects
                        .insert(vec![effect.name.clone()], effect.clone());
                }
                hir::Item::Handler(handler) => {
                    index.handlers.insert(handler.name.clone(), handler.clone());
                }
                hir::Item::TypeAlias(_) => {}
            }
        }

        index
    }

    pub fn function(&self, name: &str) -> Option<&hir::HirFunction> {
        self.functions.get(name)
    }

    pub fn contains_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub fn resolved_function(&self, defined_in: &PathBuf, name: &str) -> Option<&hir::HirFunction> {
        self.resolved_functions
            .get(defined_in)
            .and_then(|functions| functions.get(name))
    }

    pub fn struct_def(&self, path: &[String]) -> Option<&hir::HirStructDef> {
        self.structs.get(path)
    }

    pub fn contains_struct(&self, path: &[String]) -> bool {
        self.structs.contains_key(path)
    }

    pub fn enum_def(&self, path: &[String]) -> Option<&hir::HirEnumDef> {
        self.enums.get(path)
    }

    pub fn contains_enum(&self, path: &[String]) -> bool {
        self.enums.contains_key(path)
    }

    pub fn effect(&self, path: &[String]) -> Option<&hir::HirEffectDef> {
        self.effects.get(path)
    }

    pub fn contains_effect(&self, path: &[String]) -> bool {
        self.effects.contains_key(path)
    }

    pub fn handler(&self, name: &str) -> Option<&hir::HirHandlerDef> {
        self.handlers.get(name)
    }

    pub fn enum_variant(&self, path: &[String]) -> Option<&HirEnumVariantIndexEntry> {
        self.enum_variants.get(path)
    }

    pub fn enum_variant_payload(&self, path: &[String]) -> Option<&Option<Vec<hir::Ty>>> {
        self.enum_variants.get(path).map(|variant| &variant.payload)
    }
}
