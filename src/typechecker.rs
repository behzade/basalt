use std::collections::HashMap;

use crate::{
    ast::OwnedPath,
    ast_owned::OwnedItem,
    hir::{self, HirFunction, HirStructDef},
};

pub struct TypeChecker<'a> {
    ast: &'a [OwnedItem],
    context: Context,
    hir: Vec<hir::Item>,

    current_fn_return_type: Option<hir::Ty>,
}

pub struct Context {
    functions: HashMap<OwnedPath, HirFunction>,   
    structs: HashMap<OwnedPath, HirStructDef>,

    variables: Vec<HashMap<String, hir::Ty>>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    param_types: Vec<hir::Ty>,
    ret_type: hir::Ty,
}
