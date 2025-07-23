//! typechecker/patterns.rs
//!
//! This module contains the logic for checking and lowering patterns
//! from AST to HIR.

use super::{TypeChecker, TypeError};
use crate::{ast, hir};

impl<'src> TypeChecker<'src> {
    /// Checks a pattern and converts it to a `hir::Pattern`.
    /// This function handles pattern matching and adds bindings to the current scope.
    pub fn check_pattern(
        &mut self,
        pattern: &ast::Pattern<'src>,
        expected_ty: &hir::Ty<'src>,
    ) -> Result<hir::Pattern<'src>, TypeError<'src>> {
        // Debug: Print the pattern being checked
        println!("DEBUG: Checking pattern: {:?}", pattern);
        
        // Unify the pattern's expected type with the scrutinee type
        let pattern_ty = expected_ty.clone();

        let kind = match pattern {
            // Case ast::Pattern::Literal(lit):
            ast::Pattern::Literal(lit) => {
                // Determine the type of the literal
                let literal_ty = match lit {
                    &ast::Literal::Bool(_) => hir::Ty::Bool,
                    &ast::Literal::I64(_) => hir::Ty::I64,
                    &ast::Literal::F64(_) => hir::Ty::F64,
                    &ast::Literal::Str(_) => hir::Ty::Str,
                    &ast::Literal::Unit => hir::Ty::Unit,
                };
                
                // Unify this literal type with the scrutinee's type
                self.unify(&literal_ty, expected_ty)?;
                
                hir::PatternKind::Literal(lit.clone())
            }
            
            // Case ast::Pattern::Wildcard:
            ast::Pattern::Wildcard => {
                // This pattern matches anything. It doesn't add bindings.
                hir::PatternKind::Wildcard
            }
            
            // Case ast::Pattern::Identifier(name):
            ast::Pattern::Identifier(name) => {
                // This is the existing logic for variable bindings
                // First, check if this could be a nullary enum variant
                if let hir::Ty::Adt { name: enum_name, .. } = expected_ty {
                    // Check if the scrutinee type is an ADT and if this name matches a variant
                    if let Some(enum_def) = self.context.get_enum(enum_name.first().unwrap_or(&"")) {
                        if let Some(variant_info) = enum_def.variants.iter().find(|(v, _)| v == name) {
                            // Check if this variant has no fields (nullary variant)
                            if variant_info.1.is_none() || variant_info.1.as_ref().unwrap().is_empty() {
                                // This is a nullary variant pattern
                                return Ok(hir::Pattern {
                                    kind: hir::PatternKind::AdtVariant {
                                        path: vec![enum_name.first().unwrap_or(&""), name],
                                        fields: vec![],
                                    },
                                    ty: pattern_ty,
                                });
                            }
                        }
                    }
                }
                
                // If it's not a nullary variant, treat it as a binding
                self.context.add_variable(name, pattern_ty.clone());
                hir::PatternKind::Binding {
                    name,
                    is_mut: false, // For now, assume all bindings are immutable
                }
            }
            
            // Case ast::Pattern::Path { .. }:
            ast::Pattern::Path { path, args } => {
                // Handle the case where we have a single identifier as a path (e.g., "x" in "Some(x)")
                if path.len() == 1 && args.is_empty() {
                    // This is a simple identifier binding
                    let name = path[0];
                    self.context.add_variable(name, pattern_ty.clone());
                    return Ok(hir::Pattern {
                        kind: hir::PatternKind::Binding {
                            name,
                            is_mut: false,
                        },
                        ty: pattern_ty,
                    });
                }

                // This is the existing logic for AdtVariant patterns
                // Handle both qualified (Option::Some) and unqualified (Some) paths
                let (enum_name, variant_name) = if path.len() == 2 {
                    (path[0], path[1])
                } else if path.len() == 1 {
                    // For unqualified paths like `Some(x)`, search through all known enums
                    // to find the one containing this variant
                    let variant_name = path[0];
                    let (enum_name, _) = self.context
                        .find_enum_by_variant(variant_name)
                        .ok_or(TypeError::UnknownEnumVariant {
                            enum_name: "unknown",
                            variant_name,
                        })?;
                    (enum_name, variant_name)
                } else {
                    return Err(TypeError::InvalidPattern {
                        pattern: format!("{:?}", pattern),
                    });
                };

                // Look up the enum definition
                let enum_def = self
                    .context
                    .get_enum(enum_name)
                    .ok_or(TypeError::UnknownEnum(enum_name))?;

                // Find the variant and get its field types
                let variant_info = enum_def
                    .variants
                    .iter()
                    .find(|(name, _)| name == &variant_name)
                    .ok_or(TypeError::UnknownEnumVariant {
                        enum_name,
                        variant_name,
                    })?;
                let empty_vec = Vec::new();
                let variant_types = variant_info.1.as_ref().unwrap_or(&empty_vec);

                // Check that the number of pattern arguments matches the variant fields
                if args.len() != variant_types.len() {
                    return Err(TypeError::WrongArgumentCount {
                        expected: variant_types.len(),
                        found: args.len(),
                    });
                }

                // Convert variant types to HIR types first to avoid borrow issues
                let hir_variant_types: Vec<hir::Ty<'src>> = variant_types
                    .iter()
                    .map(|ty| self.lower_type(ty))
                    .collect();

                // Recursively check each sub-pattern
                let mut fields = Vec::new();
                for (arg_pattern, field_ty) in args.iter().zip(hir_variant_types.iter()) {
                    let hir_field_pattern = self.check_pattern(arg_pattern, field_ty)?;
                    fields.push(hir_field_pattern);
                }

                hir::PatternKind::AdtVariant {
                    path: path.to_vec(),
                    fields,
                }
            }
        };

        Ok(hir::Pattern {
            kind,
            ty: pattern_ty,
        })
    }
} 