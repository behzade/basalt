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
    pub functions: HashMap<String, hir::HirFunction>,
    pub resolved_functions: HashMap<(PathBuf, String), hir::HirFunction>,
    pub structs: HashMap<hir::OwnedPath, hir::HirStructDef>,
    pub enums: HashMap<hir::OwnedPath, hir::HirEnumDef>,
    pub effects: HashMap<hir::OwnedPath, hir::HirEffectDef>,
    pub handlers: HashMap<String, hir::HirHandlerDef>,
    pub enum_variants: HashMap<hir::OwnedPath, HirEnumVariantIndexEntry>,
}

impl HirIndex {
    pub fn from_items(items: &[hir::Item]) -> Self {
        let mut index = Self::default();

        for item in items {
            match item {
                hir::Item::Fn(function) => {
                    index.resolved_functions.insert(
                        (function.defined_in.clone(), function.signature.name.clone()),
                        function.clone(),
                    );
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

    pub fn enum_variant_payload(&self, path: &[String]) -> Option<&Option<Vec<hir::Ty>>> {
        self.enum_variants.get(path).map(|variant| &variant.payload)
    }
}
