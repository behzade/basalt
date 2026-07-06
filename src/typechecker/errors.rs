use crate::hir;
use crate::token::SimpleSpan;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub context: ItemContext,
}

#[derive(Debug, Clone)]
pub struct ItemContext {
    pub span: SimpleSpan,
    pub path: PathBuf,
}

impl super::checker::Typechecker {
    pub(crate) fn format_ty(ty: &hir::Ty) -> String {
        use hir::*;
        match ty {
            Ty::Special(SpecialTy::Unit) => "()".to_string(),
            Ty::Special(SpecialTy::Never) => "!".to_string(),
            Ty::Special(SpecialTy::SelfType) => "Self".to_string(),
            Ty::Primitive(PrimitiveTy::Bool) => "bool".to_string(),
            Ty::Primitive(PrimitiveTy::Byte) => "byte".to_string(),
            Ty::Primitive(PrimitiveTy::I8) => "i8".to_string(),
            Ty::Primitive(PrimitiveTy::I16) => "i16".to_string(),
            Ty::Primitive(PrimitiveTy::I32) => "i32".to_string(),
            Ty::Primitive(PrimitiveTy::I64) => "i64".to_string(),
            Ty::Primitive(PrimitiveTy::U8) => "u8".to_string(),
            Ty::Primitive(PrimitiveTy::U16) => "u16".to_string(),
            Ty::Primitive(PrimitiveTy::U32) => "u32".to_string(),
            Ty::Primitive(PrimitiveTy::U64) => "u64".to_string(),
            Ty::Primitive(PrimitiveTy::F32) => "f32".to_string(),
            Ty::Primitive(PrimitiveTy::F64) => "f64".to_string(),
            Ty::Primitive(PrimitiveTy::Str) => "str".to_string(),
            Ty::Array(elem) => format!("[{}]", Self::format_ty(elem)),
            Ty::Map { key, value } => {
                let key_str = match **key {
                    PrimitiveTy::Bool => "bool",
                    PrimitiveTy::Byte => "byte",
                    PrimitiveTy::I8 => "i8",
                    PrimitiveTy::I16 => "i16",
                    PrimitiveTy::I32 => "i32",
                    PrimitiveTy::I64 => "i64",
                    PrimitiveTy::U8 => "u8",
                    PrimitiveTy::U16 => "u16",
                    PrimitiveTy::U32 => "u32",
                    PrimitiveTy::U64 => "u64",
                    PrimitiveTy::F32 => "f32",
                    PrimitiveTy::F64 => "f64",
                    PrimitiveTy::Str => "str",
                };
                format!("Map<{}, {}>", key_str, Self::format_ty(value))
            }
            Ty::Function {
                param_types,
                ret_type,
                effects,
            } => {
                let params = param_types
                    .iter()
                    .map(Self::format_ty)
                    .collect::<Vec<_>>()
                    .join(", ");
                let eff = if effects.is_empty() {
                    String::new()
                } else {
                    let e = effects
                        .iter()
                        .map(Self::format_ty)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(" effects {{ {} }}", e)
                };
                format!("fn({}) -> {}{}", params, Self::format_ty(ret_type), eff)
            }
            Ty::Generic(name) => name.clone(),
            Ty::Adt(AdtTy::Struct { name, generics })
            | Ty::Adt(AdtTy::Enum { name, generics })
            | Ty::Adt(AdtTy::Effect { name, generics }) => {
                let path = name.join("::");
                if generics.is_empty() {
                    path
                } else {
                    let gs = generics
                        .iter()
                        .map(Self::format_ty)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}<{}>", path, gs)
                }
            }
        }
    }
}
