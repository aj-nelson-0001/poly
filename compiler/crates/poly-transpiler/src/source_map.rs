//! Source map generation for Poly-to-Rust transpilation.
//!
//! This module generates source maps that map Rust output lines back to
//! Poly source lines, enabling better debugging and error reporting.

use std::collections::HashMap;

/// A single mapping from target (Rust) line to source (Poly) line.
#[derive(Debug, Clone)]
pub struct SourceMapping {
    /// Target/source line pairs are stored independently of generated text so
    /// diagnostics can work even after formatting changes the Rust output.
    /// Line number in the target (Rust) output (1-based)
    pub target_line: usize,
    /// Line number in the source (Poly) input (1-based)
    pub source_line: usize,
    /// Optional column offset in the target line
    pub target_column: usize,
    /// Optional column offset in the source line
    pub source_column: usize,
    /// Optional name identifier for the symbol
    pub name: Option<String>,
}

/// Source map for mapping transpiled code back to original Poly source.
///
/// The current generator records coarse line mappings. The data model already
/// keeps columns and symbol locations available for a future span-precise pass.
#[derive(Debug, Clone)]
pub struct SourceMap {
    /// The map owns both texts because error rendering must remain stable even
    /// when the caller has discarded its original source buffers.
    /// The original Poly source code
    source: String,
    /// The transpiled Rust code
    target: String,
    /// Mappings from target to source
    mappings: Vec<SourceMapping>,
    /// Source line to offset mapping (line number -> byte offset)
    #[allow(dead_code)]
    source_line_offsets: Vec<usize>,
    /// Target line to offset mapping (line number -> byte offset)
    #[allow(dead_code)]
    target_line_offsets: Vec<usize>,
    /// Symbol name to source location mapping
    symbols: HashMap<String, SourceLocation>,
}

/// A location in the source code.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Length of the symbol
    pub length: usize,
}

impl SourceMap {
    /// Create a new source map from source and target code. Line offsets are
    /// precomputed once because editor diagnostics may query many positions.
    pub fn new(source: &str, target: &str) -> Self {
        let source_line_offsets = Self::compute_line_offsets(source);
        let target_line_offsets = Self::compute_line_offsets(target);

        Self {
            source: source.to_string(),
            target: target.to_string(),
            mappings: Vec::new(),
            source_line_offsets,
            target_line_offsets,
            symbols: HashMap::new(),
        }
    }

    /// Compute byte offsets for each line in the text.
    ///
    /// Offsets are cached so future position lookups can avoid rescanning the
    /// complete source or generated output for every diagnostic.
    fn compute_line_offsets(text: &str) -> Vec<usize> {
        let mut offsets = vec![0];
        for (i, _) in text.bytes().enumerate() {
            if text.as_bytes()[i] == b'\n' {
                offsets.push(i + 1);
            }
        }
        offsets
    }

    /// Add a coarse line mapping. Code generation uses this fast path for
    /// statements that do not carry column-level lowering information.
    pub fn add_mapping(&mut self, target_line: usize, source_line: usize) {
        self.mappings.push(SourceMapping {
            target_line,
            source_line,
            target_column: 0,
            source_column: 0,
            name: None,
        });
    }

    /// Add a mapping with column information.
    pub fn add_mapping_with_column(
        &mut self,
        target_line: usize,
        target_column: usize,
        source_line: usize,
        source_column: usize,
    ) {
        self.mappings.push(SourceMapping {
            target_line,
            source_line,
            target_column,
            source_column,
            name: None,
        });
    }

    /// Add a named symbol mapping.
    pub fn add_symbol(
        &mut self,
        name: &str,
        source_line: usize,
        source_column: usize,
        length: usize,
    ) {
        self.symbols.insert(
            name.to_string(),
            SourceLocation {
                line: source_line,
                column: source_column,
                length,
            },
        );
    }

    /// Look up the source location for a target line.
    /// If no exact match, returns the closest previous mapping.
    ///
    /// Generated helper lines commonly have no direct mapping, so the previous
    /// mapping gives callers the most useful surrounding Poly context.
    pub fn lookup_target_line(&self, target_line: usize) -> Option<usize> {
        // First try exact match
        if let Some(mapping) = self.mappings.iter().find(|m| m.target_line == target_line) {
            return Some(mapping.source_line);
        }
        // Fall back to closest previous mapping
        self.mappings
            .iter()
            .filter(|m| m.target_line <= target_line)
            .max_by_key(|m| m.target_line)
            .map(|m| m.source_line)
    }

    /// Look up the source location for a target position (line and column).
    /// The current lookup is line-based; retaining the column argument keeps
    /// the public API ready for a span-precise mapping pass.
    pub fn lookup_target_position(
        &self,
        target_line: usize,
        _target_column: usize,
    ) -> Option<SourceMapping> {
        // Find the closest mapping
        self.mappings
            .iter()
            .filter(|m| m.target_line <= target_line)
            .max_by_key(|m| m.target_line)
            .cloned()
    }

    /// Get source location for a symbol name.
    pub fn lookup_symbol(&self, name: &str) -> Option<&SourceLocation> {
        self.symbols.get(name)
    }

    /// Get the original source line for a target line number.
    pub fn get_source_line(&self, target_line: usize) -> Option<&str> {
        let source_line = self.lookup_target_line(target_line)?;
        self.source.lines().nth(source_line - 1)
    }

    /// Get the target line for a source line number.
    pub fn get_target_line(&self, source_line: usize) -> Option<usize> {
        self.mappings
            .iter()
            .find(|m| m.source_line == source_line)
            .map(|m| m.target_line)
    }

    /// Generate a human-readable error message with source context.
    pub fn format_error(&self, target_line: usize, message: &str) -> String {
        if let Some(source_line) = self.lookup_target_line(target_line) {
            let source_text = self.get_source_line(target_line).unwrap_or("<unknown>");
            format!(
                "Error at line {} (source line {}):\n{}\n{}^-- {}",
                target_line,
                source_line,
                source_text,
                " ".repeat(target_line.to_string().len() + 2),
                message
            )
        } else {
            format!("Error at line {}: {}", target_line, message)
        }
    }

    /// Generate a source map in JSON format (compatible with Source Map v3 spec).
    pub fn to_json(&self) -> String {
        let mappings_str = self.generate_vlq_mappings();
        let _sources: Vec<String> = self
            .mappings
            .iter()
            .map(|_| "input.poly".to_string())
            .collect();
        let names: Vec<String> = self.symbols.keys().cloned().collect();

        format!(
            r#"{{
  "version": 3,
  "file": "output.rs",
  "sourceRoot": "",
  "sources": ["input.poly"],
  "names": [{}],
  "mappings": "{}"
}}"#,
            names
                .iter()
                .map(|n| format!("\"{}\"", n))
                .collect::<Vec<_>>()
                .join(", "),
            mappings_str
        )
    }

    /// Generate VLQ-encoded mappings for Source Map v3 format.
    ///
    /// Source maps store deltas rather than absolute positions; each previous
    /// value is therefore retained while walking mappings in output order.
    fn generate_vlq_mappings(&self) -> String {
        let mut result = String::new();
        let mut prev_generated_line = 0;
        let mut prev_generated_column = 0;
        let mut prev_source_line = 0;
        let mut prev_source_column = 0;
        let mut prev_name_index = 0;

        for (i, mapping) in self.mappings.iter().enumerate() {
            if i > 0 {
                result.push(';');
            }

            // Generated line
            let generated_line = mapping.target_line - 1;
            let delta_line = generated_line as i64 - prev_generated_line as i64;
            result.push_str(&self.encode_vlq(delta_line));
            prev_generated_line = generated_line;

            // Generated column
            let generated_column = mapping.target_column;
            let delta_column = generated_column as i64 - prev_generated_column as i64;
            result.push_str(&self.encode_vlq(delta_column));
            prev_generated_column = generated_column;

            // Source index (always 0 for single source)
            result.push_str(&self.encode_vlq(0));

            // Source line
            let source_line = mapping.source_line - 1;
            let delta_source_line = source_line as i64 - prev_source_line as i64;
            result.push_str(&self.encode_vlq(delta_source_line));
            prev_source_line = source_line;

            // Source column
            let source_column = mapping.source_column;
            let delta_source_column = source_column as i64 - prev_source_column as i64;
            result.push_str(&self.encode_vlq(delta_source_column));
            prev_source_column = source_column;

            // Name index (if present)
            if let Some(name) = &mapping.name {
                if let Some(name_idx) = self.symbols.keys().position(|n| n == name) {
                    let delta_name = name_idx as i64 - prev_name_index as i64;
                    result.push_str(&self.encode_vlq(delta_name));
                    prev_name_index = name_idx;
                }
            }
        }

        result
    }

    /// Encode a signed integer as VLQ (Variable Length Quantity).
    ///
    /// The implementation mirrors the source-map specification and emits one
    /// Base64 character per five data bits after the sign is folded in.
    /// VLQ format for Source Maps:
    /// - First sextet: [continuation][4 value bits][sign bit]
    /// - Subsequent sextets: [continuation][5 value bits]
    /// - Base64 alphabet: A-Z (0-25), a-z (26-51), 0-9 (52-61), + (62), / (63)
    fn encode_vlq(&self, value: i64) -> String {
        let mut result = String::new();

        // Determine sign bit (1 for negative, 0 for positive)
        let sign_bit = if value < 0 { 1 } else { 0 };
        let mut value = value.unsigned_abs();

        // First sextet: add sign bit by shifting value left 1 and adding sign
        value = (value << 1) | sign_bit;

        loop {
            // Extract 5 bits for continuation + data
            let mut byte = (value & 0x1F) as u8;
            value >>= 5;

            // Set continuation bit if more data follows
            if value != 0 {
                byte |= 0x20; // Set bit 5 (continuation)
            }

            // Convert to Base64 character
            // Base64 alphabet: A-Z (0-25), a-z (26-51), 0-9 (52-61), + (62), / (63)
            let ch = match byte {
                0..=25 => (b'A' + byte) as char,
                26..=51 => (b'a' + (byte - 26)) as char,
                52..=61 => (b'0' + (byte - 52)) as char,
                62 => '+',
                63 => '/',
                _ => unreachable!(),
            };
            result.push(ch);

            if value == 0 {
                break;
            }
        }

        result
    }

    /// Generate a human-readable source map summary.
    pub fn summary(&self) -> String {
        let mut result = String::new();
        result.push_str("Source Map Summary:\n");
        result.push_str(&format!(
            "  Source lines: {}\n",
            self.source.lines().count()
        ));
        result.push_str(&format!(
            "  Target lines: {}\n",
            self.target.lines().count()
        ));
        result.push_str(&format!("  Mappings: {}\n", self.mappings.len()));
        result.push_str(&format!("  Symbols: {}\n", self.symbols.len()));
        result.push_str("\nMappings:\n");

        for mapping in &self.mappings {
            result.push_str(&format!(
                "  Rust line {} -> Poly line {}\n",
                mapping.target_line, mapping.source_line
            ));
        }

        result
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new("", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map_creation() {
        let source = "var x i32 := 42\nput x";
        let target = "// Generated from Poly source code\nfn main() {\n    let mut x: i32 = 42;\n    println!(\"{}\", x);\n}";

        let source_map = SourceMap::new(source, target);
        assert!(source_map.mappings.is_empty());
        assert!(source_map.source_line_offsets.len() > 1);
        assert!(source_map.target_line_offsets.len() > 1);
    }

    #[test]
    fn test_add_mapping() {
        let mut source_map = SourceMap::new("line1\nline2", "rust1\nrust2\nrust3");
        source_map.add_mapping(2, 1);

        assert_eq!(source_map.lookup_target_line(2), Some(1));
    }

    #[test]
    fn test_lookup_target_line() {
        let mut source_map = SourceMap::new("source", "target");
        source_map.add_mapping(1, 2);
        source_map.add_mapping(3, 5);

        assert_eq!(source_map.lookup_target_line(1), Some(2));
        assert_eq!(source_map.lookup_target_line(3), Some(5));
        assert_eq!(source_map.lookup_target_line(2), Some(2)); // Falls back to closest
    }

    #[test]
    fn test_format_error() {
        let mut source_map = SourceMap::new("var x := 42", "let mut x = 42;");
        source_map.add_mapping(1, 1);

        let error_msg = source_map.format_error(1, "type mismatch");
        assert!(error_msg.contains("source line 1"));
        assert!(error_msg.contains("type mismatch"));
    }

    #[test]
    fn test_symbol_lookup() {
        let mut source_map = SourceMap::new("source", "target");
        source_map.add_symbol("main", 1, 0, 4);

        let location = source_map.lookup_symbol("main");
        assert!(location.is_some());
        assert_eq!(location.unwrap().line, 1);
    }

    #[test]
    fn test_vlq_encoding() {
        let source_map = SourceMap::new("", "");

        // Test basic VLQ encoding
        assert_eq!(source_map.encode_vlq(0), "A");
        assert_eq!(source_map.encode_vlq(1), "C");
        assert_eq!(source_map.encode_vlq(-1), "D");
        assert_eq!(source_map.encode_vlq(15), "e");
    }

    #[test]
    fn test_json_generation() {
        let mut source_map = SourceMap::new("var x := 1", "let x = 1;");
        source_map.add_mapping(1, 1);
        source_map.add_symbol("x", 1, 4, 1);

        let json = source_map.to_json();
        assert!(json.contains("\"version\": 3"));
        assert!(json.contains("\"sources\": [\"input.poly\"]"));
    }

    #[test]
    fn test_summary() {
        let mut source_map = SourceMap::new("line1\nline2", "rust1\nrust2");
        source_map.add_mapping(1, 1);
        source_map.add_mapping(2, 2);

        let summary = source_map.summary();
        assert!(summary.contains("Source Map Summary"));
        assert!(summary.contains("Mappings: 2"));
    }

    #[test]
    fn test_get_source_line() {
        let source = "first line\nsecond line\nthird line";
        let mut source_map = SourceMap::new(source, "rust code");
        source_map.add_mapping(1, 2);

        assert_eq!(source_map.get_source_line(1), Some("second line"));
    }

    #[test]
    fn test_get_target_line() {
        let mut source_map = SourceMap::new("source", "rust1\nrust2\nrust3");
        source_map.add_mapping(2, 1);
        source_map.add_mapping(3, 2);

        assert_eq!(source_map.get_target_line(1), Some(2));
        assert_eq!(source_map.get_target_line(2), Some(3));
    }
}
