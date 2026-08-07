//! Poly Language Type System
//!
//! Defines the type system for the Poly programming language.
//! Provides type inference, checking, and semantic analysis.

use std::collections::HashMap;

/// A Poly type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolyType {
    /// Primitive types
    I8, U8, I16, U16, I32, U32, I64, U64, I128, U128,
    F32, F64, ISize, USize,
    Bool, Char,
    
    /// String types
    String, UnicodeString,
    
    /// Byte types
    Byte, Bytes,
    
    /// Compound types
    Array(Box<PolyType>, usize),
    Tuple(Vec<PolyType>),
    Vec(Box<PolyType>),
    Option(Box<PolyType>),
    Result(Box<PolyType>, Box<PolyType>),
    
    /// Reference types
    Reference(bool, Box<PolyType>), // (mutable, inner)
    
    /// Function type
    Function {
        params: Vec<PolyType>,
        ret: Box<PolyType>,
    },
    
    /// User-defined type (by name)
    Named(String),
    
    /// Type variable (for inference)
    TypeVar(usize),
    
    /// Unknown type
    Unknown,
}

impl std::fmt::Display for PolyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolyType::I8 => write!(f, "i8"),
            PolyType::U8 => write!(f, "u8"),
            PolyType::I16 => write!(f, "i16"),
            PolyType::U16 => write!(f, "u16"),
            PolyType::I32 => write!(f, "i32"),
            PolyType::U32 => write!(f, "u32"),
            PolyType::I64 => write!(f, "i64"),
            PolyType::U64 => write!(f, "u64"),
            PolyType::I128 => write!(f, "i128"),
            PolyType::U128 => write!(f, "u128"),
            PolyType::F32 => write!(f, "f32"),
            PolyType::F64 => write!(f, "f64"),
            PolyType::ISize => write!(f, "isize"),
            PolyType::USize => write!(f, "usize"),
            PolyType::Bool => write!(f, "bool"),
            PolyType::Char => write!(f, "char"),
            PolyType::String => write!(f, "String"),
            PolyType::UnicodeString => write!(f, "String"),
            PolyType::Byte => write!(f, "u8"),
            PolyType::Bytes => write!(f, "Vec<u8>"),
            PolyType::Array(inner, size) => write!(f, "[{}; {}]", inner, size),
            PolyType::Tuple(types) => {
                write!(f, "(")?;
                for (i, t) in types.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            PolyType::Vec(inner) => write!(f, "Vec<{}>", inner),
            PolyType::Option(inner) => write!(f, "Option<{}>", inner),
            PolyType::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            PolyType::Reference(mutable, inner) => {
                if *mutable { write!(f, "&mut {}", inner) }
                else { write!(f, "&{}", inner) }
            }
            PolyType::Function { params, ret } => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            PolyType::Named(name) => write!(f, "{}", name),
            PolyType::TypeVar(id) => write!(f, "?{}", id),
            PolyType::Unknown => write!(f, "_"),
        }
    }
}

/// A type error.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Option<(usize, usize)>,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.span {
            Some((start, end)) => write!(f, "Type error at {}..{}: {}", start, end, self.message),
            None => write!(f, "Type error: {}", self.message),
        }
    }
}

impl std::error::Error for TypeError {}

/// The type checker / type system.
pub struct TypeSystem {
    /// Type environment (variable name -> type)
    env: HashMap<String, PolyType>,
    /// Type variables for inference
    type_vars: HashMap<usize, PolyType>,
    /// Next type variable ID
    next_var_id: usize,
    /// Collected errors
    errors: Vec<TypeError>,
}

impl TypeSystem {
    /// Create a new type system instance.
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
            type_vars: HashMap::new(),
            next_var_id: 0,
            errors: Vec::new(),
        }
    }
    
    /// Create a fresh type variable.
    pub fn fresh_type_var(&mut self) -> PolyType {
        let id = self.next_var_id;
        self.next_var_id += 1;
        PolyType::TypeVar(id)
    }
    
    /// Register a variable with a type.
    pub fn register_var(&mut self, name: String, ty: PolyType) {
        self.env.insert(name, ty);
    }
    
    /// Look up a variable's type.
    pub fn lookup_var(&self, name: &str) -> Option<&PolyType> {
        self.env.get(name)
    }
    
    /// Unify two types (check compatibility).
    pub fn unify(&mut self, left: &PolyType, right: &PolyType) -> Result<PolyType, TypeError> {
        match (left, right) {
            // Same types
            (a, b) if a == b => Ok(a.clone()),
            
            // Type variables
            (PolyType::TypeVar(id), ty) | (ty, PolyType::TypeVar(id)) => {
                self.type_vars.insert(*id, ty.clone());
                Ok(ty.clone())
            }
            
            // Numeric type compatibility
            (PolyType::I32, PolyType::I64) | (PolyType::I64, PolyType::I32) => {
                Ok(PolyType::I64)
            }
            (PolyType::F32, PolyType::F64) | (PolyType::F64, PolyType::F32) => {
                Ok(PolyType::F64)
            }
            
            // Option unwrapping
            (PolyType::Option(inner), ty) | (ty, PolyType::Option(inner)) => {
                self.unify(inner, ty)
            }
            
            // Tuple types
            (PolyType::Tuple(a), PolyType::Tuple(b)) if a.len() == b.len() => {
                let unified: Result<Vec<_>, _> = a.iter().zip(b.iter())
                    .map(|(x, y)| self.unify(x, y))
                    .collect();
                Ok(PolyType::Tuple(unified?))
            }
            
            // Vec types
            (PolyType::Vec(a), PolyType::Vec(b)) => {
                Ok(PolyType::Vec(Box::new(self.unify(a, b)?)))
            }
            
            // Named types (same name)
            (PolyType::Named(a), PolyType::Named(b)) if a == b => {
                Ok(PolyType::Named(a.clone()))
            }
            
            _ => Err(TypeError {
                message: format!("Cannot unify {} with {}", left, right),
                span: None,
            }),
        }
    }
    
    /// Infer the type of a binary operation.
    pub fn infer_binary_op(&mut self, op: &str, left: &PolyType, right: &PolyType) -> Result<PolyType, TypeError> {
        match op {
            // Arithmetic
            "+" | "-" | "*" | "/" | "%" => {
                // Both operands must be the same numeric type
                let unified = self.unify(left, right)?;
                Ok(unified)
            }
            // Comparison
            "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                // Both operands must be comparable
                self.unify(left, right)?;
                Ok(PolyType::Bool)
            }
            // Logical
            "&&" | "||" => {
                self.unify(left, &PolyType::Bool)?;
                self.unify(right, &PolyType::Bool)?;
                Ok(PolyType::Bool)
            }
            // Bitwise
            "&" | "|" | "^" | "<<" | ">>" => {
                let unified = self.unify(left, right)?;
                Ok(unified)
            }
            _ => Err(TypeError {
                message: format!("Unknown operator: {}", op),
                span: None,
            }),
        }
    }
    
    /// Check if a type is numeric.
    pub fn is_numeric(ty: &PolyType) -> bool {
        matches!(ty,
            PolyType::I8 | PolyType::U8 | PolyType::I16 | PolyType::U16 |
            PolyType::I32 | PolyType::U32 | PolyType::I64 | PolyType::U64 |
            PolyType::I128 | PolyType::U128 | PolyType::F32 | PolyType::F64 |
            PolyType::ISize | PolyType::USize
        )
    }
    
    /// Check if a type is coercible to another.
    pub fn is_coercible(&self, from: &PolyType, to: &PolyType) -> bool {
        match (from, to) {
            (a, b) if a == b => true,
            (PolyType::I32, PolyType::I64) => true,
            (PolyType::F32, PolyType::F64) => true,
            (PolyType::I32, PolyType::F64) => true,
            (PolyType::Option(inner), ty) => self.is_coercible(inner, ty),
            _ => false,
        }
    }
    
    /// Get collected errors.
    pub fn errors(&self) -> &[TypeError] {
        &self.errors
    }
    
    /// Clear all errors.
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }
    
    /// Get the current environment.
    pub fn env(&self) -> &HashMap<String, PolyType> {
        &self.env
    }
}

impl Default for TypeSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_system_creation() {
        let ts = TypeSystem::new();
        assert!(ts.env.is_empty());
        assert!(ts.errors.is_empty());
    }

    #[test]
    fn test_type_unification() {
        let mut ts = TypeSystem::new();
        
        // Same types
        let result = ts.unify(&PolyType::I32, &PolyType::I32);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PolyType::I32);
        
        // Different types should fail
        let result = ts.unify(&PolyType::I32, &PolyType::Bool);
        assert!(result.is_err());
    }

    #[test]
    fn test_type_var_unification() {
        let mut ts = TypeSystem::new();
        let var = ts.fresh_type_var();
        
        // Unify type var with concrete type
        let result = ts.unify(&var, &PolyType::I32);
        assert!(result.is_ok());
    }

    #[test]
    fn test_binary_op_inference() {
        let mut ts = TypeSystem::new();
        
        // Addition of same types
        let result = ts.infer_binary_op("+", &PolyType::I32, &PolyType::I32);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PolyType::I32);
        
        // Comparison returns bool
        let result = ts.infer_binary_op("==", &PolyType::I32, &PolyType::I32);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PolyType::Bool);
    }

    #[test]
    fn test_is_numeric() {
        assert!(TypeSystem::is_numeric(&PolyType::I32));
        assert!(TypeSystem::is_numeric(&PolyType::F64));
        assert!(!TypeSystem::is_numeric(&PolyType::Bool));
        assert!(!TypeSystem::is_numeric(&PolyType::String));
    }

    #[test]
    fn test_coercion() {
        let ts = TypeSystem::new();
        
        // Same type is coercible
        assert!(ts.is_coercible(&PolyType::I32, &PolyType::I32));
        
        // Numeric widening
        assert!(ts.is_coercible(&PolyType::I32, &PolyType::I64));
        assert!(ts.is_coercible(&PolyType::F32, &PolyType::F64));
        
        // Not coercible
        assert!(!ts.is_coercible(&PolyType::I32, &PolyType::Bool));
    }

    #[test]
    fn test_type_display() {
        assert_eq!(PolyType::I32.to_string(), "i32");
        assert_eq!(PolyType::Bool.to_string(), "bool");
        assert_eq!(PolyType::Vec(Box::new(PolyType::I32)).to_string(), "Vec<i32>");
        assert_eq!(
            PolyType::Tuple(vec![PolyType::I32, PolyType::Bool]).to_string(),
            "(i32, bool)"
        );
        assert_eq!(
            PolyType::Result(Box::new(PolyType::I32), Box::new(PolyType::String)).to_string(),
            "Result<i32, String>"
        );
    }

    #[test]
    fn test_tuple_unification() {
        let mut ts = TypeSystem::new();
        
        let a = PolyType::Tuple(vec![PolyType::I32, PolyType::Bool]);
        let b = PolyType::Tuple(vec![PolyType::I32, PolyType::Bool]);
        
        let result = ts.unify(&a, &b);
        assert!(result.is_ok());
        
        // Different lengths
        let c = PolyType::Tuple(vec![PolyType::I32]);
        let result = ts.unify(&a, &c);
        assert!(result.is_err());
    }

    #[test]
    fn test_vec_unification() {
        let mut ts = TypeSystem::new();
        
        let a = PolyType::Vec(Box::new(PolyType::I32));
        let b = PolyType::Vec(Box::new(PolyType::I32));
        
        let result = ts.unify(&a, &b);
        assert!(result.is_ok());
    }
}
