//! Pure type-classification and generic-substitution helpers.
//!
//! These functions depend only on [`PolyType`] and the parser AST.  They are
//! used throughout the [`TypeChecker`](super::TypeChecker) implementation but
//! have no dependency on checker state, so they live here to keep the main
//! checker file focused on the state machine.

use poly_types::PolyType;

#[cfg(test)]
use poly_parser::ast::TypeAnnotation;

// ---------------------------------------------------------------------------
// Generic type operations
// ---------------------------------------------------------------------------

/// Unify a declared generic type against a concrete argument type, recording
/// each type-variable binding (`T -> i32`) in `bindings`.
///
/// Only declared types that mention a generic parameter (directly or inside a
/// container like `Vec<T>`) produce bindings; unrelated shapes are ignored so
/// compatibility checking can report the mismatch.
pub(crate) fn unify_declared(
    declared: &PolyType,
    actual: &PolyType,
    generics: &[String],
    bindings: &mut std::collections::HashMap<String, PolyType>,
) {
    if let PolyType::Named(name) = declared {
        if generics.iter().any(|generic| generic == name) {
            bindings.insert(name.clone(), actual.clone());
            return;
        }
    }
    match (declared, actual) {
        (PolyType::Vec(a), PolyType::Vec(b)) => unify_declared(a, b, generics, bindings),
        (PolyType::Map(ak, av), PolyType::Map(bk, bv)) => {
            unify_declared(ak, bk, generics, bindings);
            unify_declared(av, bv, generics, bindings);
        }
        (PolyType::Set(a), PolyType::Set(b)) => unify_declared(a, b, generics, bindings),
        (PolyType::Option(a), PolyType::Option(b)) => unify_declared(a, b, generics, bindings),
        (PolyType::Result(a_ok, a_err), PolyType::Result(b_ok, b_err)) => {
            unify_declared(a_ok, b_ok, generics, bindings);
            unify_declared(a_err, b_err, generics, bindings);
        }
        (PolyType::Array(a, _), PolyType::Array(b, _)) => unify_declared(a, b, generics, bindings),
        (PolyType::Tuple(a), PolyType::Tuple(b)) if a.len() == b.len() => {
            for (a, b) in a.iter().zip(b.iter()) {
                unify_declared(a, b, generics, bindings);
            }
        }
        (PolyType::Reference(_, a), PolyType::Reference(_, b)) => {
            unify_declared(a, b, generics, bindings)
        }
        (
            PolyType::Function {
                params: a_params,
                ret: a_ret,
            },
            PolyType::Function {
                params: b_params,
                ret: b_ret,
            },
        ) => {
            for (a, b) in a_params.iter().zip(b_params.iter()) {
                unify_declared(a, b, generics, bindings);
            }
            unify_declared(a_ret, b_ret, generics, bindings);
        }
        _ => {}
    }
}

/// Replace type-variable markers in `ty` with the concrete types gathered at a
/// call site. Unbound variables stay as `Named(name)` markers so downstream
/// checks treat them as opaque polymorphic types (same as the `Unknown`
/// lowering used inside generic bodies).
pub(crate) fn substitute_generic(
    ty: &PolyType,
    generics: &[String],
    bindings: &std::collections::HashMap<String, PolyType>,
) -> PolyType {
    match ty {
        PolyType::Named(name) => {
            if generics.iter().any(|generic| generic == name) {
                bindings.get(name).cloned().unwrap_or_else(|| ty.clone())
            } else {
                ty.clone()
            }
        }
        PolyType::Vec(inner) => {
            PolyType::Vec(Box::new(substitute_generic(inner, generics, bindings)))
        }
        PolyType::Map(key, value) => PolyType::Map(
            Box::new(substitute_generic(key, generics, bindings)),
            Box::new(substitute_generic(value, generics, bindings)),
        ),
        PolyType::Set(inner) => {
            PolyType::Set(Box::new(substitute_generic(inner, generics, bindings)))
        }
        PolyType::Option(inner) => {
            PolyType::Option(Box::new(substitute_generic(inner, generics, bindings)))
        }
        PolyType::Result(ok, err) => PolyType::Result(
            Box::new(substitute_generic(ok, generics, bindings)),
            Box::new(substitute_generic(err, generics, bindings)),
        ),
        PolyType::Array(inner, size) => PolyType::Array(
            Box::new(substitute_generic(inner, generics, bindings)),
            *size,
        ),
        PolyType::Tuple(types) => PolyType::Tuple(
            types
                .iter()
                .map(|inner| substitute_generic(inner, generics, bindings))
                .collect(),
        ),
        PolyType::Reference(mutable, inner) => PolyType::Reference(
            *mutable,
            Box::new(substitute_generic(inner, generics, bindings)),
        ),
        PolyType::Function { params, ret } => PolyType::Function {
            params: params
                .iter()
                .map(|inner| substitute_generic(inner, generics, bindings))
                .collect(),
            ret: Box::new(substitute_generic(ret, generics, bindings)),
        },
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// Type classification
// ---------------------------------------------------------------------------

/// Check whether two types are assignment-compatible (the right side may be
/// assigned to a variable of the left side's type).
pub(crate) fn compatible(actual: &PolyType, expected: &PolyType) -> bool {
    if matches!(actual, PolyType::Unknown) || matches!(expected, PolyType::Unknown) {
        return true;
    }
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        (a, b) if is_numeric(a) && is_numeric(b) => numeric_rank(a) <= numeric_rank(b),
        // An empty array literal is polymorphic and may initialize any container.
        (PolyType::Vec(inner), _) if matches!(**inner, PolyType::Unknown) => true,
        (PolyType::Array(a, _), PolyType::Array(b, _)) => compatible(a, b),
        (PolyType::Vec(a), PolyType::Vec(b)) => {
            compatible(a, b) || (is_numeric(a) && is_numeric(b))
        }
        (PolyType::Map(a_key, a_value), PolyType::Map(b_key, b_value)) => {
            compatible(a_key, b_key) && compatible(a_value, b_value)
        }
        (PolyType::Set(a), PolyType::Set(b)) => compatible(a, b),
        (PolyType::Tuple(a), PolyType::Tuple(b)) if a.len() == b.len() => {
            a.iter().zip(b.iter()).all(|(a, b)| compatible(a, b))
        }
        (PolyType::Option(a), PolyType::Option(b)) => compatible(a, b),
        (PolyType::Result(a_ok, a_err), PolyType::Result(b_ok, b_err)) => {
            compatible(a_ok, b_ok) && compatible(a_err, b_err)
        }
        (PolyType::Reference(a_mut, a), PolyType::Reference(b_mut, b)) => {
            (!*b_mut || *a_mut) && compatible(a, b)
        }
        (
            PolyType::Function {
                params: a_params,
                ret: a_ret,
            },
            PolyType::Function {
                params: b_params,
                ret: b_ret,
            },
        ) => {
            a_params.len() == b_params.len()
                && a_params
                    .iter()
                    .zip(b_params.iter())
                    .all(|(a, b)| compatible(a, b))
                && compatible(a_ret, b_ret)
        }
        _ => false,
    }
}

/// Widen two numeric types to their common wider type.
pub(crate) fn numeric_join(left: &PolyType, right: &PolyType) -> PolyType {
    if numeric_rank(left) >= numeric_rank(right) {
        left.clone()
    } else {
        right.clone()
    }
}

/// Numeric rank for type widening: higher rank wins in a binary operation.
pub(crate) fn numeric_rank(ty: &PolyType) -> u8 {
    match ty {
        PolyType::I8 | PolyType::U8 => 1,
        PolyType::I16 | PolyType::U16 => 2,
        PolyType::I32 | PolyType::U32 => 3,
        PolyType::I64 | PolyType::U64 => 4,
        PolyType::I128 | PolyType::U128 => 5,
        PolyType::ISize | PolyType::USize => 4,
        PolyType::F32 => 6,
        PolyType::F64 => 7,
        _ => 0,
    }
}

/// True when `ty` is any numeric type (integer or float).
pub(crate) fn is_numeric(ty: &PolyType) -> bool {
    numeric_rank(ty) > 0
}

/// True when `ty` is an integer type (not float).
pub(crate) fn is_integer(ty: &PolyType) -> bool {
    matches!(
        ty,
        PolyType::I8
            | PolyType::U8
            | PolyType::I16
            | PolyType::U16
            | PolyType::I32
            | PolyType::U32
            | PolyType::I64
            | PolyType::U64
            | PolyType::I128
            | PolyType::U128
            | PolyType::ISize
            | PolyType::USize
    )
}

/// Extract the element type from a container type (`Vec<T>`, `Map<K, V>`,
/// `String` → `Char`).
pub(crate) fn element_type(ty: &PolyType) -> Option<PolyType> {
    match ty {
        PolyType::Array(inner, _) | PolyType::Vec(inner) => Some((**inner).clone()),
        PolyType::Map(_, value) => Some((**value).clone()),
        PolyType::String => Some(PolyType::Char),
        _ => None,
    }
}

/// True when a value of type `actual` can be cast `as` to `target`.
pub(crate) fn is_castable(actual: &PolyType, target: &PolyType) -> bool {
    matches!(actual, PolyType::Unknown)
        || matches!(target, PolyType::Unknown)
        || (is_numeric(actual) && is_numeric(target))
        || (actual == &PolyType::Char && is_integer(target))
        || (is_integer(actual) && target == &PolyType::Char)
        || actual == target
}

// ---------------------------------------------------------------------------
// AST helpers
// ---------------------------------------------------------------------------

/// Split a loop binding into names. `loop: (a, b) in collection` carries the
/// tuple pattern in the loop variable; every other binding is a plain name.
pub(crate) fn split_loop_binding(variable: &str) -> Vec<String> {
    let trimmed = variable.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        trimmed[1..trimmed.len() - 1]
            .split(',')
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect()
    } else {
        vec![trimmed.to_string()]
    }
}

/// Convert a [`TypeAnnotation`] (parser AST) to a [`PolyType`] (checker type
/// system).  Generic containers like `Vec<T>`, `Map<K, V>`, `Option<T>`,
/// `Result<T, E>`, function types, and references are all lowered.
#[cfg(test)]
pub(crate) fn annotation_type(annotation: &TypeAnnotation) -> PolyType {
    match annotation {
        TypeAnnotation::Named(name) => match name.as_str() {
            "i8" => PolyType::I8,
            "u8" => PolyType::U8,
            "i16" => PolyType::I16,
            "u16" => PolyType::U16,
            "i32" => PolyType::I32,
            "u32" => PolyType::U32,
            "i64" => PolyType::I64,
            "u64" => PolyType::U64,
            "i128" => PolyType::I128,
            "u128" => PolyType::U128,
            "f32" => PolyType::F32,
            "f64" => PolyType::F64,
            "isize" => PolyType::ISize,
            "usize" => PolyType::USize,
            "bool" => PolyType::Bool,
            "char" => PolyType::Char,
            "string" | "String" => PolyType::String,
            "uchar" => PolyType::Char,
            "ustring" => PolyType::UnicodeString,
            "byte" => PolyType::Byte,
            "bytes" => PolyType::Bytes,
            "Self" => PolyType::Named("Self".into()),
            _ => PolyType::Named(name.clone()),
        },
        TypeAnnotation::Vec(inner) => PolyType::Vec(Box::new(annotation_type(inner))),
        TypeAnnotation::Option(inner) => PolyType::Option(Box::new(annotation_type(inner))),
        TypeAnnotation::Result(ok, err) => PolyType::Result(
            Box::new(annotation_type(ok)),
            Box::new(annotation_type(err)),
        ),
        TypeAnnotation::Array(inner, _) => PolyType::Vec(Box::new(annotation_type(inner))),
        TypeAnnotation::Tuple(types) => {
            PolyType::Tuple(types.iter().map(annotation_type).collect())
        }
        TypeAnnotation::Reference(mutable, inner) => {
            PolyType::Reference(*mutable, Box::new(annotation_type(inner)))
        }
        TypeAnnotation::Pointer(inner) => {
            PolyType::Named(format!("ptr {}", annotation_type(inner)))
        }
        TypeAnnotation::Nullable(inner) => PolyType::Option(Box::new(annotation_type(inner))),
        TypeAnnotation::Function { params, ret } => PolyType::Function {
            params: params.iter().map(annotation_type).collect(),
            ret: Box::new(annotation_type(ret)),
        },
        TypeAnnotation::Generic { name, args } => {
            let arg_types: Vec<PolyType> = args.iter().map(annotation_type).collect();
            match name.as_str() {
                "Vec" => {
                    if let Some(inner) = arg_types.into_iter().next() {
                        PolyType::Vec(Box::new(inner))
                    } else {
                        PolyType::Unknown
                    }
                }
                "Map" => {
                    if arg_types.len() == 2 {
                        PolyType::Map(
                            Box::new(arg_types[0].clone()),
                            Box::new(arg_types[1].clone()),
                        )
                    } else {
                        PolyType::Unknown
                    }
                }
                "Set" => {
                    if let Some(inner) = arg_types.into_iter().next() {
                        PolyType::Set(Box::new(inner))
                    } else {
                        PolyType::Unknown
                    }
                }
                "Option" => {
                    if let Some(inner) = arg_types.into_iter().next() {
                        PolyType::Option(Box::new(inner))
                    } else {
                        PolyType::Unknown
                    }
                }
                "Result" => {
                    if arg_types.len() == 2 {
                        PolyType::Result(
                            Box::new(arg_types[0].clone()),
                            Box::new(arg_types[1].clone()),
                        )
                    } else {
                        PolyType::Unknown
                    }
                }
                "Box" => {
                    if let Some(inner) = arg_types.into_iter().next() {
                        PolyType::Named(format!("Box<{}>", inner))
                    } else {
                        PolyType::Unknown
                    }
                }
                _ => PolyType::Named(name.clone()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poly_parser::ast::TypeAnnotation;

    #[test]
    fn is_numeric_classifies_types() {
        assert!(is_numeric(&PolyType::I32));
        assert!(is_numeric(&PolyType::F64));
        assert!(!is_numeric(&PolyType::Bool));
        assert!(!is_numeric(&PolyType::String));
    }

    #[test]
    fn is_integer_excludes_floats() {
        assert!(is_integer(&PolyType::I32));
        assert!(is_integer(&PolyType::U64));
        assert!(!is_integer(&PolyType::F32));
        assert!(!is_integer(&PolyType::Bool));
    }

    #[test]
    fn numeric_join_widens() {
        assert_eq!(numeric_join(&PolyType::I32, &PolyType::I64), PolyType::I64);
        assert_eq!(numeric_join(&PolyType::F64, &PolyType::I32), PolyType::F64);
        assert_eq!(numeric_join(&PolyType::I32, &PolyType::I32), PolyType::I32);
    }

    #[test]
    fn compatible_uses_unknown_as_wildcard() {
        assert!(compatible(&PolyType::Unknown, &PolyType::I32));
        assert!(compatible(&PolyType::I32, &PolyType::Unknown));
    }

    #[test]
    fn compatible_allows_numeric_widening() {
        // Wider actual type is NOT compatible with narrower expected type.
        assert!(!compatible(&PolyType::I64, &PolyType::I32));
        // Narrower actual type IS compatible with wider expected type.
        assert!(compatible(&PolyType::I32, &PolyType::I64));
    }

    #[test]
    fn split_loop_binding_single_name() {
        assert_eq!(split_loop_binding("x"), vec!["x"]);
    }

    #[test]
    fn split_loop_binding_tuple() {
        assert_eq!(split_loop_binding("(a, b)"), vec!["a", "b"]);
    }

    #[test]
    fn split_loop_binding_tuple_with_spaces() {
        assert_eq!(split_loop_binding("  ( a , b )  "), vec!["a", "b"]);
    }

    #[test]
    fn annotation_type_primitives() {
        assert_eq!(
            annotation_type(&TypeAnnotation::Named("i32".into())),
            PolyType::I32
        );
        assert_eq!(
            annotation_type(&TypeAnnotation::Named("bool".into())),
            PolyType::Bool
        );
        assert_eq!(
            annotation_type(&TypeAnnotation::Named("string".into())),
            PolyType::String
        );
    }

    #[test]
    fn annotation_type_containers() {
        let vec_i32 = TypeAnnotation::Vec(Box::new(TypeAnnotation::Named("i32".into())));
        assert_eq!(
            annotation_type(&vec_i32),
            PolyType::Vec(Box::new(PolyType::I32))
        );

        let opt = TypeAnnotation::Option(Box::new(TypeAnnotation::Named("i32".into())));
        assert_eq!(
            annotation_type(&opt),
            PolyType::Option(Box::new(PolyType::I32))
        );
    }

    #[test]
    fn annotation_type_generic_map() {
        let map = TypeAnnotation::Generic {
            name: "Map".into(),
            args: vec![
                TypeAnnotation::Named("string".into()),
                TypeAnnotation::Named("i32".into()),
            ],
        };
        assert_eq!(
            annotation_type(&map),
            PolyType::Map(Box::new(PolyType::String), Box::new(PolyType::I32))
        );
    }
}
