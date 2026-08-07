//! Poly Language Transpiler
//!
//! Generates Rust code from Poly source code.
//! This crate is a stub — the full transpiler will be implemented in a future milestone.

/// The Poly-to-Rust transpiler.
pub struct Transpiler;

impl Transpiler {
    /// Create a new transpiler instance.
    pub fn new() -> Self {
        Self
    }

    /// Transpile Poly source code to Rust code.
    pub fn transpile(&self, source: &str) -> Result<String, String> {
        // TODO: Implement full transpiler
        Ok(format!("// Transpiled from Poly\nfn main() {{\n    // TODO: Implement transpilation\n}}"))
    }
}

impl Default for Transpiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpiler_stub() {
        let t = Transpiler::new();
        let result = t.transpile("put \"hello\"");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("fn main()"));
    }
}
