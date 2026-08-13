//! Poly Language Server.
//!
//! A dependency-free Language Server Protocol implementation for the Poly
//! programming language.  The server provides:
//!
//! - full-text document synchronization (`didOpen`, `didChange`, `didSave`,
//!   `didClose`)
//! - diagnostics from the lexer, parser (with error recovery), and type
//!   checker, with source ranges
//! - keyword and symbol completion
//! - hover information for declarations and keywords
//! - document symbols (outline)
//!
//! The JSON-RPC transport runs in the `poly-lsp` binary; the [`server::Server`]
//! state machine is exposed as a library so it can be tested and embedded.

pub mod json;
pub mod server;
