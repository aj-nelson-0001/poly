//! Poly Language Type System
//!
//! Defines the type system for the Poly programming language.
//! This crate is a stub — the full type system will be implemented in a future milestone.

/// The Poly language type system.
pub struct TypeSystem;

impl TypeSystem {
    /// Create a new type system instance.
    pub fn new() -> Self {
        Self
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
    fn test_type_system_stub() {
        let ts = TypeSystem::new();
        // Type system stub works
        drop(ts);
    }
}
