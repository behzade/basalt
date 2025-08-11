use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::path::PathBuf;

use crate::ast_owned::{
    OwnedEnumDef, OwnedExpr, OwnedFunction, OwnedHandlerDef, OwnedImplBlock, OwnedImportPath,
    OwnedItem, OwnedItemWithSpan, OwnedLiteral, OwnedMethod, OwnedPattern, OwnedRecordField,
    OwnedSatisfiesBlock, OwnedStructDef, OwnedTraitDef, OwnedTraitMethod, OwnedType,
    OwnedTypeAliasBody, OwnedTypeAliasDef, SpannedExpr, SpannedPattern, SpannedStmt,
};
use crate::hir;

//===============================//
// Helpers
//===============================//

fn indent(buf: &mut String, level: usize) {
    let _ = write!(buf, "{:\u{20}<1}", "").map(|_| ()); // no-op to keep type
    for _ in 0..level {
        buf.push_str("  ");
    }
}

fn join_path(parts: &[String]) -> String {
    if parts.is_empty() {
        String::from("<anon>")
    } else {
        parts.join("::")
    }
}

fn format_type(ty: &OwnedType) -> String {
    let mut s = join_path(&ty.path);
    if !ty.generics.is_empty() {
        let inner: Vec<String> = ty.generics.iter().map(format_type).collect();
        s.push('<');
        s.push_str(&inner.join(", "));
        s.push('>');
    }
    s
}

fn format_lit(lit: &OwnedLiteral) -> String {
    match lit {
        OwnedLiteral::Bool(b) => b.to_string(),
        OwnedLiteral::I8(v) => v.to_string(),
        OwnedLiteral::I16(v) => v.to_string(),
        OwnedLiteral::I32(v) => v.to_string(),
        OwnedLiteral::I64(v) => v.to_string(),
        OwnedLiteral::U8(v) => v.to_string(),
        OwnedLiteral::U16(v) => v.to_string(),
        OwnedLiteral::U32(v) => v.to_string(),
        OwnedLiteral::U64(v) => v.to_string(),
        OwnedLiteral::F32(v) => v.to_string(),
        OwnedLiteral::F64(v) => v.to_string(),
        OwnedLiteral::Str(s) => format!("\"{}\"", s),
        OwnedLiteral::Unit => String::from("()"),
    }
}

fn write_expr(buf: &mut String, e: &SpannedExpr, level: usize) {
    match &e.item {
        OwnedExpr::Literal(l) => {
            buf.push_str(&format_lit(l));
        }
        OwnedExpr::Path(p) => buf.push_str(&p.join("::")),
        OwnedExpr::FieldAccess { receiver, field } => {
            write_expr(buf, receiver, level);
            buf.push('.');
            buf.push_str(field);
        }
        OwnedExpr::Unary { op, rhs } => {
            use crate::ast::UnaryOp;
            let sym = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            buf.push_str(sym);
            write_expr(buf, rhs, level);
        }
        OwnedExpr::MethodCall { receiver, method, args } => {
            write_expr(buf, receiver, level);
            buf.push('.');
            buf.push_str(method);
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 { buf.push_str(", "); }
                write_expr(buf, a, level);
            }
            buf.push(')');
        }
        OwnedExpr::Binary { op, lhs, rhs } => {
            buf.push('(');
            write_expr(buf, lhs, level);
            buf.push(' ');
            use std::fmt::Write as _;
            let _ = write!(buf, "{}", op);
            buf.push(' ');
            write_expr(buf, rhs, level);
            buf.push(')');
        }
        OwnedExpr::Call { fun, args } => {
            write_expr(buf, fun, level);
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                write_expr(buf, a, level);
            }
            buf.push(')');
        }
        OwnedExpr::StructInit { path, generics: _, fields } => {
            buf.push_str(&path.join("::"));
            buf.push_str(" { ");
            for (i, (name, val)) in fields.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                buf.push_str(name);
                buf.push_str(": ");
                write_expr(buf, val, level);
            }
            buf.push_str(" }");
        }
        OwnedExpr::Block { stmts, last_expr } => {
            buf.push_str("{");
            if !stmts.is_empty() {
                buf.push('\n');
            }
            for s in stmts {
                indent(buf, level + 1);
                write_stmt(buf, s, level + 1);
                buf.push('\n');
            }
            if let Some(expr) = last_expr {
                indent(buf, level + 1);
                write_expr(buf, expr, level + 1);
                buf.push('\n');
            }
            indent(buf, level);
            buf.push('}');
        }
        OwnedExpr::If { cond, then_block, else_block } => {
            buf.push_str("if ");
            write_expr(buf, cond, level);
            buf.push(' ');
            write_expr(buf, then_block, level);
            if let Some(else_b) = else_block {
                buf.push_str(" else ");
                write_expr(buf, else_b, level);
            }
        }
        OwnedExpr::Match { scrutinee, arms } => {
            buf.push_str("match ");
            write_expr(buf, scrutinee, level);
            buf.push_str(" {\n");
            for (pat, expr) in arms {
                indent(buf, level + 1);
                write_pattern(buf, pat);
                buf.push_str(" => ");
                write_expr(buf, expr, level + 1);
                buf.push_str(",\n");
            }
            indent(buf, level);
            buf.push('}');
        }
        OwnedExpr::While { cond, body } => {
            buf.push_str("while ");
            write_expr(buf, cond, level);
            buf.push(' ');
            write_expr(buf, body, level);
        }
        OwnedExpr::Perform { path, args } => {
            buf.push_str(&format!("perform {}(", path.join("::")));
            for (i, a) in args.iter().enumerate() {
                if i > 0 { buf.push_str(", "); }
                write_expr(buf, a, level);
            }
            buf.push(')');
        }
        OwnedExpr::Handle { body, handler: _ } => {
            buf.push_str("handle ");
            write_expr(buf, body, level);
            buf.push_str(" with <handler>");
        }
        OwnedExpr::Cast { expr, ty } => {
            buf.push('(');
            write_expr(buf, expr, level);
            buf.push_str(") as ");
            buf.push_str(&format_type(ty));
            buf.push(')');
        }
        OwnedExpr::Array(items) => {
            buf.push('[');
            for (i, it) in items.iter().enumerate() {
                if i > 0 { buf.push_str(", "); }
                write_expr(buf, it, level);
            }
            buf.push(']');
        }
        OwnedExpr::Map(entries) => {
            buf.push_str("map{");
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 { buf.push_str(", "); }
                write_expr(buf, k, level);
                buf.push_str(": ");
                write_expr(buf, v, level);
            }
            buf.push('}');
        }
        OwnedExpr::Error => buf.push_str("<error>"),
    }
}

fn write_pattern(buf: &mut String, p: &SpannedPattern) {
    match &p.item {
        OwnedPattern::Literal(l) => buf.push_str(&format_lit(l)),
        OwnedPattern::Identifier(n) => buf.push_str(n),
        OwnedPattern::Path { path, args } => {
            buf.push_str(&path.join("::"));
            if !args.is_empty() {
                buf.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_pattern(buf, a);
                }
                buf.push(')');
            }
        }
        OwnedPattern::Wildcard => buf.push('_'),
    }
}

fn write_stmt(buf: &mut String, s: &SpannedStmt, level: usize) {
    match &s.item {
        crate::ast_owned::OwnedStmt::Let { is_mut, name, ty, value } => {
            buf.push_str("let ");
            if *is_mut { buf.push_str("mut "); }
            buf.push_str(name);
            if let Some(ty) = ty { buf.push_str(": "); buf.push_str(&format_type(ty)); }
            if let Some(v) = value { buf.push_str(" = "); write_expr(buf, v, level); }
            buf.push(';');
        }
        crate::ast_owned::OwnedStmt::Return(expr) => {
            buf.push_str("return");
            if let Some(e) = expr { buf.push(' '); write_expr(buf, e, level); }
            buf.push(';');
        }
        crate::ast_owned::OwnedStmt::Assign(lhs, rhs) => {
            write_expr(buf, lhs, level);
            buf.push_str(" = ");
            write_expr(buf, rhs, level);
            buf.push(';');
        }
        crate::ast_owned::OwnedStmt::Expr(e) => {
            write_expr(buf, e, level);
            buf.push(';');
        }
        crate::ast_owned::OwnedStmt::Error => buf.push_str("<error>"),
    }
}

fn write_import(buf: &mut String, import: &OwnedImportPath) {
    buf.push_str("import ");
    buf.push_str(&import.path.join("::"));
    if let Some(alias) = &import.alias {
        buf.push_str(" as ");
        buf.push_str(alias);
    }
}

fn write_type_alias(buf: &mut String, ta: &OwnedTypeAliasDef, level: usize) {
    if ta.is_public { buf.push_str("pub "); }
    buf.push_str("type ");
    buf.push_str(&ta.name);
    buf.push_str(" = ");
    match &ta.aliased {
        OwnedTypeAliasBody::Type(t) => buf.push_str(&format_type(t)),
        OwnedTypeAliasBody::Record(fields) => {
            buf.push_str("{ ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 { buf.push_str(", "); }
                if f.is_public { buf.push_str("pub "); }
                buf.push_str(&f.name);
                buf.push_str(": ");
                buf.push_str(&format_type(&f.ty));
            }
            buf.push_str(" }");
        }
        OwnedTypeAliasBody::Union(variants) => {
            for (i, (name, ot)) in variants.iter().enumerate() {
                if i > 0 { buf.push_str(" | "); }
                buf.push_str(name);
                if let Some(t) = ot { buf.push('('); buf.push_str(&format_type(t)); buf.push(')'); }
            }
        }
    }
    buf.push(';');
}

fn write_function(buf: &mut String, f: &OwnedFunction, level: usize) {
    if f.is_public { buf.push_str("pub "); }
    buf.push_str("fn ");
    buf.push_str(&f.name);
    if !f.generics.is_empty() {
        buf.push('<');
        buf.push_str(&f.generics.join(", "));
        buf.push('>');
    }
    buf.push('(');
    for (i, (name, ty)) in f.params.iter().enumerate() {
        if i > 0 { buf.push_str(", "); }
        if let Some(n) = name { buf.push_str(n); buf.push_str(": "); }
        buf.push_str(&format_type(ty));
    }
    buf.push(')');
    if let Some(ret) = &f.ret_type { buf.push_str(" -> "); buf.push_str(&format_type(ret)); }
    if !f.effects.is_empty() {
        buf.push_str(" ![");
        buf.push_str(&f.effects.join(", "));
        buf.push(']');
    }
    buf.push_str(" {\n");
    indent(buf, level + 1);
    write_expr(buf, &f.body, level + 1);
    buf.push('\n');
    indent(buf, level);
    buf.push('}');
}

fn write_struct(buf: &mut String, s: &OwnedStructDef) {
    if s.is_public { buf.push_str("pub "); }
    buf.push_str("struct ");
    buf.push_str(&s.name);
    buf.push_str(" { ");
    for (i, (name, ty)) in s.fields.iter().enumerate() {
        if i > 0 { buf.push_str(", "); }
        buf.push_str(name);
        buf.push_str(": ");
        buf.push_str(&format_type(ty));
    }
    buf.push_str(" }");
}

fn write_enum(buf: &mut String, e: &OwnedEnumDef) {
    if e.is_public { buf.push_str("pub "); }
    buf.push_str("enum ");
    buf.push_str(e.name.as_deref().unwrap_or("<anon>"));
    buf.push_str(" { ");
    for (i, (name, payload)) in e.variants.iter().enumerate() {
        if i > 0 { buf.push_str(", "); }
        buf.push_str(name);
        if let Some(ts) = payload {
            buf.push('(');
            for (j, t) in ts.iter().enumerate() {
                if j > 0 { buf.push_str(", "); }
                buf.push_str(&format_type(t));
            }
            buf.push(')');
        }
    }
    buf.push_str(" }");
}

fn write_trait(buf: &mut String, t: &OwnedTraitDef, level: usize) {
    if t.is_public { buf.push_str("pub "); }
    buf.push_str("trait ");
    buf.push_str(&t.name);
    buf.push_str(" {\n");
    for m in &t.methods {
        indent(buf, level + 1);
        write_trait_method(buf, m);
        buf.push_str(";\n");
    }
    buf.push('}');
}

fn write_trait_method(buf: &mut String, m: &OwnedTraitMethod) {
    buf.push_str(&m.name);
    buf.push('(');
    for (i, (name, ty)) in m.params.iter().enumerate() {
        if i > 0 { buf.push_str(", "); }
        if let Some(n) = name { buf.push_str(n); buf.push_str(": "); }
        buf.push_str(&format_type(ty));
    }
    buf.push(')');
    if let Some(ret) = &m.ret_type { buf.push_str(" -> "); buf.push_str(&format_type(ret)); }
}

fn write_effect(buf: &mut String, e: &OwnedHandlerDef, level: usize) {
    if e.is_public { buf.push_str("pub "); }
    buf.push_str("handler ");
    buf.push_str(&e.name);
    if !e.effects.is_empty() {
        buf.push_str(" for [");
        buf.push_str(&e.effects.join(", "));
        buf.push(']');
    }
    buf.push_str(" {\n");
    for f in &e.functions {
        indent(buf, level + 1);
        write_function(buf, f, level + 1);
        buf.push_str("\n");
    }
    buf.push('}');
}

fn write_impl(buf: &mut String, imp: &OwnedImplBlock, level: usize) {
    buf.push_str("impl ");
    buf.push_str(&format_type(&imp.target_type));
    if let Some(trait_path) = &imp.interface {
        buf.push_str(" : ");
        buf.push_str(&trait_path.join("::"));
    }
    buf.push_str(" {\n");
    for m in &imp.methods {
        indent(buf, level + 1);
        write_function(buf, m, level + 1);
        buf.push_str("\n");
    }
    buf.push('}');
}

fn write_satisfies(buf: &mut String, s: &OwnedSatisfiesBlock, level: usize) {
    buf.push_str("satisfies ");
    buf.push_str(&format_type(&s.target_type));
    buf.push_str(" : ");
    buf.push_str(&s.trait_names.join(", "));
    if let Some(ms) = &s.methods {
        buf.push_str(" {\n");
        for m in ms {
            indent(buf, level + 1);
            write_function(buf, m, level + 1);
            buf.push_str("\n");
        }
        buf.push('}');
    } else {
        buf.push(';');
    }
}

//===============================//
// Public API: AST
//===============================//

pub fn compact_ast_to_string(ast: &HashMap<PathBuf, Vec<OwnedItemWithSpan>>) -> String {
    let mut buf = String::new();
    let mut files: Vec<_> = ast.keys().cloned().collect();
    files.sort();
    for (fi, file) in files.iter().enumerate() {
        if fi > 0 { buf.push_str("\n"); }
        let path_str = file.to_string_lossy();
        let _ = writeln!(&mut buf, "{}:", path_str);
        if let Some(items) = ast.get(file) {
            for item in items {
                indent(&mut buf, 1);
                write_owned_item(&mut buf, &item.item, 1);
                buf.push_str("\n");
            }
        }
    }
    buf
}

// Compact debug-like printer: preserve Rust Debug structure but with fewer newlines, trimmed spans
pub fn compact_ast_debug_to_string(ast: &HashMap<PathBuf, Vec<OwnedItemWithSpan>>) -> String {
    fn fmt_span(span: &crate::token::SimpleSpan) -> String {
        format!("{}..{}", span.start, span.end)
    }

    fn fmt_owned_item_with_span(item: &OwnedItemWithSpan) -> String {
        // Keep Spanned{ item: <...>, span: a..b } but compress inner structures
        format!("Spanned{{ item: {}, span: {} }}", fmt_owned_item(&item.item), fmt_span(&item.span))
    }

    fn fmt_owned_item(item: &OwnedItem) -> String {
        match item {
            OwnedItem::ImportBlock { imports } => {
                let inner = imports
                    .iter()
                    .map(|i| {
                        let alias = i.alias.as_ref().map(|a| format!(", alias: {}", a)).unwrap_or_default();
                        format!(
                            "OwnedImportPath{{ path: [{}]{} }}",
                            i.path.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", "),
                            alias
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("ImportBlock{{ imports: [{}] }}", inner)
            }
            OwnedItem::TypeAlias(ta) => {
                let aliased = match &ta.aliased {
                    OwnedTypeAliasBody::Type(t) => format!("Type({})", format_type(t)),
                    OwnedTypeAliasBody::Record(fs) => format!(
                        "Record([{}])",
                        fs.iter()
                            .map(|f| format!("(name: \"{}\", ty: {}, is_public: {})", f.name, format_type(&f.ty), f.is_public))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    OwnedTypeAliasBody::Union(vs) => format!(
                        "Union([{}])",
                        vs.iter()
                            .map(|(n, ot)| match ot { Some(t) => format!("(\"{}\", Some({}))", n, format_type(t)), None => format!("(\"{}\", None)", n) })
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                format!(
                    "TypeAlias(OwnedTypeAliasDef{{ name: \"{}\", generics: [{}], aliased: {}, is_public: {} }})",
                    ta.name,
                    ta.generics.iter().map(|g| format!("\"{}\"", g)).collect::<Vec<_>>().join(", "),
                    aliased,
                    ta.is_public
                )
            }
            OwnedItem::Fn(f) => format!(
                "Fn(OwnedFunction{{ name: \"{}\" }})",
                f.name
            ),
            OwnedItem::Method(m) => format!(
                "Method(OwnedMethod{{ type_name: \"{}\", name: \"{}\" }})",
                m.type_name, m.name
            ),
            OwnedItem::Struct(s) => format!(
                "Struct(OwnedStructDef{{ name: \"{}\" }})",
                s.name
            ),
            OwnedItem::Enum(e) => format!(
                "Enum(OwnedEnumDef{{ name: {} }})",
                e.name.as_ref().map(|n| format!("\"{}\"", n)).unwrap_or("None".to_string())
            ),
            OwnedItem::Trait(t) => format!(
                "Trait(OwnedTraitDef{{ name: \"{}\" }})",
                t.name
            ),
            OwnedItem::Effect(_) => "Effect(..)".to_string(),
            OwnedItem::Handler(h) => format!(
                "Handler(OwnedHandlerDef{{ name: \"{}\" }})",
                h.name
            ),
            OwnedItem::Satisfies(_) => "Satisfies(..)".to_string(),
            OwnedItem::Impl(_) => "Impl(..)".to_string(),
            OwnedItem::Stmt(_) => "Stmt(..)".to_string(),
        }
    }

    let mut buf = String::new();
    let mut files: Vec<_> = ast.keys().cloned().collect();
    files.sort();
    for (fi, file) in files.iter().enumerate() {
        if fi > 0 { buf.push_str("\n"); }
        let items = &ast[file];
        let inner = items.iter().map(fmt_owned_item_with_span).collect::<Vec<_>>().join(",\n  ");
        buf.push_str(&format!("{{\n  \"{}\": [\n  {}\n  ]\n}}", file.to_string_lossy(), inner));
        if fi + 1 < files.len() { buf.push_str("\n"); }
    }
    buf
}

fn write_owned_item(buf: &mut String, item: &OwnedItem, level: usize) {
    match item {
        OwnedItem::ImportBlock { imports } => {
            for (i, imp) in imports.iter().enumerate() {
                if i > 0 { buf.push_str("\n"); indent(buf, level); }
                write_import(buf, imp);
            }
        }
        OwnedItem::TypeAlias(ta) => write_type_alias(buf, ta, level),
        OwnedItem::Fn(f) => write_function(buf, f, level),
        OwnedItem::Method(m) => write_method(buf, m, level),
        OwnedItem::Struct(s) => write_struct(buf, s),
        OwnedItem::Enum(e) => write_enum(buf, e),
        OwnedItem::Trait(t) => write_trait(buf, t, level),
        OwnedItem::Effect(_) => buf.push_str("<effect>") ,
        OwnedItem::Handler(h) => write_effect(buf, h, level),
        OwnedItem::Satisfies(s) => write_satisfies(buf, s, level),
        OwnedItem::Impl(i) => write_impl(buf, i, level),
        OwnedItem::Stmt(s) => write_stmt(buf, s, level),
    }
}

fn write_method(buf: &mut String, m: &OwnedMethod, level: usize) {
    if m.is_public { buf.push_str("pub "); }
    buf.push_str("method ");
    buf.push_str(&m.type_name);
    buf.push_str("::");
    buf.push_str(&m.name);
    buf.push('(');
    for (i, (name, ty)) in m.params.iter().enumerate() {
        if i > 0 { buf.push_str(", "); }
        if let Some(n) = name { buf.push_str(n); buf.push_str(": "); }
        buf.push_str(&format_type(ty));
    }
    buf.push(')');
    if let Some(ret) = &m.ret_type { buf.push_str(" -> "); buf.push_str(&format_type(ret)); }
    if !m.effects.is_empty() {
        buf.push_str(" ![");
        buf.push_str(&m.effects.join(", "));
        buf.push(']');
    }
    buf.push_str(" {\n");
    indent(buf, level + 1);
    write_expr(buf, &m.body, level + 1);
    buf.push('\n');
    indent(buf, level);
    buf.push('}');
}

//===============================//
// Public API: HIR (concise)
//===============================//

pub fn compact_hir_to_string(items: &[hir::Item]) -> String {
    fn fmt_ty(ty: &hir::Ty) -> String {
        use hir::{AdtTy, PrimitiveTy, SpecialTy, Ty};
        match ty {
            Ty::Special(SpecialTy::Unit) => "()".to_string(),
            Ty::Special(SpecialTy::Never) => "!".to_string(),
            Ty::Special(SpecialTy::SelfType) => "Self".to_string(),
            Ty::Primitive(PrimitiveTy::Bool) => "bool".to_string(),
            Ty::Primitive(PrimitiveTy::Byte) => "byte".to_string(),
            Ty::Primitive(PrimitiveTy::I32) => "i32".to_string(),
            Ty::Primitive(PrimitiveTy::I64) => "i64".to_string(),
            Ty::Primitive(PrimitiveTy::F64) => "f64".to_string(),
            Ty::Primitive(PrimitiveTy::Str) => "str".to_string(),
            Ty::Adt(AdtTy::Struct { name, generics })
            | Ty::Adt(AdtTy::Enum { name, generics })
            | Ty::Adt(AdtTy::Trait { name, generics })
            | Ty::Adt(AdtTy::Effect { name, generics }) => {
                let mut s = name.join("::");
                if !generics.is_empty() {
                    let inner: Vec<String> = generics.iter().map(fmt_ty).collect();
                    s.push('<');
                    s.push_str(&inner.join(", "));
                    s.push('>');
                }
                s
            }
            Ty::Array(inner) => format!("[{}]", fmt_ty(inner)),
            Ty::Map { key, value } => format!("map<{}, {}>", format!("{:?}", key), fmt_ty(value)),
            Ty::Function { param_types, ret_type, effects } => {
                let params = param_types.iter().map(fmt_ty).collect::<Vec<_>>().join(", ");
                let mut s = format!("fn({}) -> {}", params, fmt_ty(ret_type));
                if !effects.is_empty() {
                    s.push_str(" ![");
                    s.push_str(&effects.iter().map(fmt_ty).collect::<Vec<_>>().join(", "));
                    s.push(']');
                }
                s
            }
            Ty::Generic(n) => n.clone(),
        }
    }

    fn fmt_unary(op: hir::UnaryOp) -> &'static str {
        match op { hir::UnaryOp::Negate => "-", hir::UnaryOp::Not => "!" }
    }

    fn fmt_binary(op: hir::BinaryOp) -> &'static str {
        use hir::BinaryOp::*;
        match op {
            Add => "+", Sub => "-", Mul => "*", Div => "/", Mod => "%",
            Eq => "==", Ne => "!=", Lt => "<", Lte => "<=", Gt => ">", Gte => ">=",
            Assign => "=", And => "&&", Or => "||", Xor => "^",
            BitShiftLeft => "<<", BitShiftRight => ">>",
        }
    }

    fn write_hir_expr(buf: &mut String, e: &hir::Expr, level: usize, show_types: bool) {
        match &e.kind {
            hir::ExprKind::Literal(_, text) => {
                // Quote strings heuristically
                if matches!(e.ty, hir::Ty::Primitive(hir::PrimitiveTy::Str)) {
                    buf.push('"');
                    buf.push_str(text);
                    buf.push('"');
                } else {
                    buf.push_str(text);
                }
            }
            hir::ExprKind::Array(items) => {
                buf.push('[');
                for (i, it) in items.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_hir_expr(buf, it, level, show_types);
                }
                buf.push(']');
            }
            hir::ExprKind::Map(entries) => {
                buf.push_str("map{");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_hir_expr(buf, k, level, show_types);
                    buf.push_str(": ");
                    write_hir_expr(buf, v, level, show_types);
                }
                buf.push('}');
            }
            hir::ExprKind::Path(p) => buf.push_str(&p.join("::")),
            hir::ExprKind::FieldAccess { receiver, field } => {
                write_hir_expr(buf, receiver, level, show_types);
                buf.push('.');
                buf.push_str(field);
            }
            hir::ExprKind::Unary { op, rhs } => {
                buf.push_str(fmt_unary(*op));
                write_hir_expr(buf, rhs, level, show_types);
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                buf.push('(');
                write_hir_expr(buf, lhs, level, show_types);
                buf.push(' ');
                buf.push_str(fmt_binary(*op));
                buf.push(' ');
                write_hir_expr(buf, rhs, level, show_types);
                buf.push(')');
            }
            hir::ExprKind::Call { fun, args } => {
                write_hir_expr(buf, fun, level, show_types);
                buf.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_hir_expr(buf, a, level, show_types);
                }
                buf.push(')');
            }
            hir::ExprKind::StructInit { path, fields } => {
                buf.push_str(&path.join("::"));
                buf.push_str(" { ");
                for (i, (name, val)) in fields.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    buf.push_str(name);
                    buf.push_str(": ");
                    write_hir_expr(buf, val, level, show_types);
                }
                buf.push_str(" }");
            }
            hir::ExprKind::Block(block) => {
                buf.push_str("{\n");
                for s in &block.stmts {
                    indent(buf, level + 1);
                    write_hir_stmt(buf, s, level + 1, show_types);
                    buf.push('\n');
                }
                if let Some(last) = &block.last_expr {
                    indent(buf, level + 1);
                    write_hir_expr(buf, last, level + 1, show_types);
                    buf.push('\n');
                }
                indent(buf, level);
                buf.push('}');
            }
            hir::ExprKind::If { cond, then_block, else_block } => {
                buf.push_str("if ");
                write_hir_expr(buf, cond, level, show_types);
                buf.push(' ');
                // then_block is a HirBlock
                write_hir_block(buf, then_block, level, show_types);
                if let Some(e) = else_block {
                    buf.push_str(" else ");
                    write_hir_expr(buf, e, level, show_types);
                }
            }
            hir::ExprKind::Match { scrutinee, arms } => {
                buf.push_str("match ");
                write_hir_expr(buf, scrutinee, level, show_types);
                buf.push_str(" {\n");
                for (pat, expr) in arms {
                    indent(buf, level + 1);
                    write!(buf, "{:?}", pat).ok(); // TODO: pretty pattern
                    buf.push_str(" => ");
                    write_hir_expr(buf, expr, level + 1, show_types);
                    buf.push_str(",\n");
                }
                indent(buf, level);
                buf.push('}');
            }
            hir::ExprKind::While { cond, body } => {
                buf.push_str("while ");
                write_hir_expr(buf, cond, level, show_types);
                buf.push(' ');
                write_hir_block(buf, body, level, show_types);
            }
            hir::ExprKind::Perform { path, args } => {
                buf.push_str("perform ");
                buf.push_str(&path.join("::"));
                buf.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_hir_expr(buf, a, level, show_types);
                }
                buf.push(')');
            }
            hir::ExprKind::Handle { body, handler } => {
                buf.push_str("handle ");
                write_hir_block(buf, body, level, show_types);
                buf.push_str(" with ");
                match handler {
                    hir::HirHandlerBody::Path(p) => buf.push_str(&p.join("::")),
                    hir::HirHandlerBody::Inline(funcs) => {
                        buf.push_str("inline { /* ");
                        buf.push_str(&funcs.len().to_string());
                        buf.push_str(" functions */ }");
                    }
                }
            }
            hir::ExprKind::Cast { expr } => {
                buf.push('(');
                write_hir_expr(buf, expr, level, show_types);
                buf.push_str(") as ");
                buf.push_str(&fmt_ty(&e.ty));
                buf.push(')');
            }
            hir::ExprKind::Error => buf.push_str("<error>"),
        }
        if show_types {
            buf.push_str(" : ");
            buf.push_str(&fmt_ty(&e.ty));
        }
    }

    fn write_hir_stmt(buf: &mut String, s: &hir::Stmt, level: usize, show_types: bool) {
        match s {
            hir::Stmt::Let { name, value, ty, is_mut, .. } => {
                buf.push_str("let ");
                if *is_mut { buf.push_str("mut "); }
                buf.push_str(name);
                buf.push_str(": ");
                buf.push_str(&fmt_ty(ty));
                if let Some(v) = value {
                    buf.push_str(" = ");
                    write_hir_expr(buf, v, level, show_types);
                }
                buf.push(';');
            }
            hir::Stmt::Return { value, .. } => {
                buf.push_str("return");
                if let Some(e) = value {
                    buf.push(' ');
                    write_hir_expr(buf, e, level, show_types);
                }
                buf.push(';');
            }
            hir::Stmt::Assign { lhs, rhs, .. } => {
                write_hir_expr(buf, lhs, level, show_types);
                buf.push_str(" = ");
                write_hir_expr(buf, rhs, level, show_types);
                buf.push(';');
            }
            hir::Stmt::Expr { expr, .. } => {
                write_hir_expr(buf, expr, level, show_types);
                buf.push(';');
            }
            hir::Stmt::Error { .. } => buf.push_str("<error>"),
        }
    }

    fn write_hir_block(buf: &mut String, b: &hir::HirBlock, level: usize, show_types: bool) {
        buf.push_str("{\n");
        for s in &b.stmts {
            indent(buf, level + 1);
            write_hir_stmt(buf, s, level + 1, show_types);
            buf.push('\n');
        }
        if let Some(last) = &b.last_expr {
            indent(buf, level + 1);
            write_hir_expr(buf, last, level + 1, show_types);
            buf.push('\n');
        }
        indent(buf, level);
        buf.push('}');
    }

    let mut buf = String::new();
    for item in items {
        match item {
            hir::Item::Fn(f) => {
                let params = f
                    .signature
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, fmt_ty(&p.ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    &mut buf,
                    "fn {}({}) -> {} {{",
                    f.signature.name, params, fmt_ty(&f.signature.ret_type)
                );
                for s in &f.body.stmts {
                    indent(&mut buf, 1);
                    write_hir_stmt(&mut buf, s, 1, false);
                    buf.push('\n');
                }
                if let Some(last) = &f.body.last_expr {
                    indent(&mut buf, 1);
                    write_hir_expr(&mut buf, last, 1, false);
                    buf.push('\n');
                }
                buf.push_str("}\n");
            }
            hir::Item::Struct(s) => {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, fmt_ty(&f.ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(&mut buf, "struct {} {{ {} }}", s.name, fields);
            }
            hir::Item::Enum(e) => {
                let variants = e
                    .variants
                    .iter()
                    .map(|v| match &v.payload {
                        Some(ts) => format!(
                            "{}({})",
                            v.name,
                            ts.iter().map(|t| fmt_ty(t)).collect::<Vec<_>>().join(", ")
                        ),
                        None => v.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let _ = writeln!(&mut buf, "enum {} {{ {} }}", e.name, variants);
            }
            hir::Item::TypeAlias(ta) => {
                let _ = writeln!(&mut buf, "type {} = {};", ta.name, fmt_ty(&ta.aliased));
            }
            hir::Item::Trait(t) => {
                let methods = t
                    .methods
                    .iter()
                    .map(|m| {
                        format!(
                            "{}({}) -> {}",
                            m.name,
                            m.params
                                .iter()
                                .map(|p| format!("{}: {}", p.name, fmt_ty(&p.ty)))
                                .collect::<Vec<_>>()
                                .join(", "),
                            fmt_ty(&m.ret_type)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(&mut buf, "trait {} {{ {} }}", t.name, methods);
            }
            hir::Item::Effect(eff) => {
                let ops = eff
                    .operations
                    .iter()
                    .map(|op| {
                        format!(
                            "{}({}) -> {}",
                            op.name,
                            op.params
                                .iter()
                                .map(|p| fmt_ty(&p.ty))
                                .collect::<Vec<_>>()
                                .join(", "),
                            fmt_ty(&op.ret_type)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(&mut buf, "effect {} {{ {} }}", eff.name, ops);
            }
            hir::Item::Impl(imp) => {
                let trait_path = imp
                    .trait_path
                    .as_ref()
                    .map(|p| p.join("::"))
                    .unwrap_or_else(|| "<inherent>".to_string());
                let _ = writeln!(&mut buf, "impl {} for {} {{ ... }}", trait_path, fmt_ty(&imp.target_type));
            }
            hir::Item::Handler(h) => {
                let _ = writeln!(&mut buf, "handler {} {{ ... }}", h.name);
            }
        }
    }
    buf
}

// Compact debug-like HIR printer: keeps Debug shapes but compresses/one-line items and includes bodies
pub fn compact_hir_debug_to_string(items: &[hir::Item]) -> String {
    // For now, reuse the pretty form that includes bodies but format types using Debug
    fn fmt_ty_dbg(ty: &hir::Ty) -> String { format!("{:?}", ty) }

    fn write_expr(buf: &mut String, e: &hir::Expr, level: usize) {
        match &e.kind {
            hir::ExprKind::Literal(_, text) => buf.push_str(text),
            hir::ExprKind::Array(items) => {
                buf.push('[');
                for (i, it) in items.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_expr(buf, it, level);
                }
                buf.push(']');
            }
            hir::ExprKind::Map(entries) => {
                buf.push_str("map{");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { buf.push_str(", "); }
                    write_expr(buf, k, level);
                    buf.push_str(": ");
                    write_expr(buf, v, level);
                }
                buf.push('}');
            }
            hir::ExprKind::Path(p) => buf.push_str(&p.join("::")),
            hir::ExprKind::FieldAccess { receiver, field } => { write_expr(buf, receiver, level); buf.push('.'); buf.push_str(field); }
            hir::ExprKind::Unary { op, rhs } => { buf.push_str(&format!("{:?}", op)); write_expr(buf, rhs, level); }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                buf.push('('); write_expr(buf, lhs, level); buf.push(' '); buf.push_str(&format!("{:?}", op)); buf.push(' '); write_expr(buf, rhs, level); buf.push(')');
            }
            hir::ExprKind::Call { fun, args } => {
                write_expr(buf, fun, level); buf.push('(');
                for (i, a) in args.iter().enumerate() { if i > 0 { buf.push_str(", "); } write_expr(buf, a, level); }
                buf.push(')');
            }
            hir::ExprKind::StructInit { path, fields } => {
                buf.push_str(&path.join("::")); buf.push_str(" { ");
                for (i, (name, val)) in fields.iter().enumerate() { if i > 0 { buf.push_str(", "); } buf.push_str(name); buf.push_str(": "); write_expr(buf, val, level); }
                buf.push_str(" }");
            }
            hir::ExprKind::Block(block) => {
                buf.push_str("{\n");
                for s in &block.stmts { indent(buf, level + 1); write_stmt(buf, s, level + 1); buf.push('\n'); }
                if let Some(last) = &block.last_expr { indent(buf, level + 1); write_expr(buf, last, level + 1); buf.push('\n'); }
                indent(buf, level); buf.push('}');
            }
            hir::ExprKind::If { cond, then_block, else_block } => {
                buf.push_str("if "); write_expr(buf, cond, level); buf.push(' '); write_block(buf, then_block, level);
                if let Some(e) = else_block { buf.push_str(" else "); write_expr(buf, e, level); }
            }
            hir::ExprKind::Match { scrutinee, arms } => {
                buf.push_str("match "); write_expr(buf, scrutinee, level); buf.push_str(" {\n");
                for (pat, expr) in arms { indent(buf, level + 1); write!(buf, "{:?}", pat).ok(); buf.push_str(" => "); write_expr(buf, expr, level + 1); buf.push_str(",\n"); }
                indent(buf, level); buf.push('}');
            }
            hir::ExprKind::While { cond, body } => { buf.push_str("while "); write_expr(buf, cond, level); buf.push(' '); write_block(buf, body, level); }
            hir::ExprKind::Perform { path, args } => { buf.push_str("perform "); buf.push_str(&path.join("::")); buf.push('('); for (i, a) in args.iter().enumerate() { if i > 0 { buf.push_str(", "); } write_expr(buf, a, level); } buf.push(')'); }
            hir::ExprKind::Handle { body, handler } => { buf.push_str("handle "); write_block(buf, body, level); buf.push_str(" with "); match handler { hir::HirHandlerBody::Path(p) => buf.push_str(&p.join("::")), hir::HirHandlerBody::Inline(funcs) => { buf.push_str("inline{" ); buf.push_str(&funcs.len().to_string()); buf.push_str("}" ); } } }
            hir::ExprKind::Cast { expr } => { buf.push('('); write_expr(buf, expr, level); buf.push_str(") as "); buf.push_str(&fmt_ty_dbg(&e.ty)); buf.push(')'); }
            hir::ExprKind::Error => buf.push_str("<error>"),
        }
    }

    fn write_stmt(buf: &mut String, s: &hir::Stmt, level: usize) {
        match s {
            hir::Stmt::Let { name, value, ty, is_mut, .. } => {
                buf.push_str("let "); if *is_mut { buf.push_str("mut "); } buf.push_str(name); buf.push_str(": "); buf.push_str(&fmt_ty_dbg(ty));
                if let Some(v) = value { buf.push_str(" = "); write_expr(buf, v, level); } buf.push(';');
            }
            hir::Stmt::Return { value, .. } => { buf.push_str("return"); if let Some(e) = value { buf.push(' '); write_expr(buf, e, level); } buf.push(';'); }
            hir::Stmt::Assign { lhs, rhs, .. } => { write_expr(buf, lhs, level); buf.push_str(" = "); write_expr(buf, rhs, level); buf.push(';'); }
            hir::Stmt::Expr { expr, .. } => { write_expr(buf, expr, level); buf.push(';'); }
            hir::Stmt::Error { .. } => buf.push_str("<error>"),
        }
    }

    fn write_block(buf: &mut String, b: &hir::HirBlock, level: usize) {
        buf.push_str("{\n");
        for s in &b.stmts { indent(buf, level + 1); write_stmt(buf, s, level + 1); buf.push('\n'); }
        if let Some(last) = &b.last_expr { indent(buf, level + 1); write_expr(buf, last, level + 1); buf.push('\n'); }
        indent(buf, level); buf.push('}');
    }

    let mut buf = String::new();
    for item in items {
        match item {
            hir::Item::Fn(f) => {
                let params = f.signature.params.iter().map(|p| format!("{}: {}", p.name, fmt_ty_dbg(&p.ty))).collect::<Vec<_>>().join(", ");
                let _ = writeln!(&mut buf, "Fn(HirFunction {{ name: {}, params: [{}], ret: {} }})", f.signature.name, params, fmt_ty_dbg(&f.signature.ret_type));
                write_block(&mut buf, &f.body, 0);
                buf.push('\n');
            }
            hir::Item::Struct(s) => { let fields = s.fields.iter().map(|f| format!("{}: {}", f.name, fmt_ty_dbg(&f.ty))).collect::<Vec<_>>().join(", "); let _ = writeln!(&mut buf, "Struct({} {{ {} }})", s.name, fields); }
            hir::Item::Enum(e) => { let variants = e.variants.iter().map(|v| match &v.payload { Some(ts) => format!("{}({})", v.name, ts.iter().map(|t| fmt_ty_dbg(t)).collect::<Vec<_>>().join(", ")), None => v.name.clone(), }).collect::<Vec<_>>().join(" | "); let _ = writeln!(&mut buf, "Enum({} {{ {} }})", e.name, variants); }
            hir::Item::TypeAlias(ta) => { let _ = writeln!(&mut buf, "TypeAlias({} = {})", ta.name, fmt_ty_dbg(&ta.aliased)); }
            hir::Item::Trait(t) => { let methods = t.methods.iter().map(|m| format!("{}({}) -> {}", m.name, m.params.iter().map(|p| format!("{}: {}", p.name, fmt_ty_dbg(&p.ty))).collect::<Vec<_>>().join(", "), fmt_ty_dbg(&m.ret_type))).collect::<Vec<_>>().join(", "); let _ = writeln!(&mut buf, "Trait({} {{ {} }})", t.name, methods); }
            hir::Item::Effect(eff) => { let ops = eff.operations.iter().map(|op| format!("{}({}) -> {}", op.name, op.params.iter().map(|p| fmt_ty_dbg(&p.ty)).collect::<Vec<_>>().join(", "), fmt_ty_dbg(&op.ret_type))).collect::<Vec<_>>().join(", "); let _ = writeln!(&mut buf, "Effect({} {{ {} }})", eff.name, ops); }
            hir::Item::Impl(imp) => { let tr = imp.trait_path.as_ref().map(|p| p.join("::")).unwrap_or_else(|| "<inherent>".to_string()); let _ = writeln!(&mut buf, "Impl({} for {})", tr, fmt_ty_dbg(&imp.target_type)); }
            hir::Item::Handler(h) => { let _ = writeln!(&mut buf, "Handler({})", h.name); }
        }
    }
    buf
}


