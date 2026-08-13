//! Poly language server binary.
//!
//! Speaks JSON-RPC 2.0 over stdio with `Content-Length` framing, the transport
//! convention used by most editors.  The server itself lives in the `server`
//! module so it can be embedded and unit-tested without a pipe.

use std::io::{self, BufRead, Read, Write};

use poly_lsp::json::{self, Json};
use poly_lsp::server::Server;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server::new();

    // Read Content-Length framed messages until EOF (client closed the pipe)
    // or the server dispatches an `exit` notification.
    while let Some(content) = read_message(&stdin) {
        let message = match json::parse(&content) {
            Ok(message) => message,
            Err(error) => {
                // Report parse failures as JSON-RPC errors so editors surface
                // them instead of silently losing the connection.
                let error_response = Json::obj(vec![
                    ("jsonrpc", Json::str("2.0")),
                    ("id", Json::Null),
                    (
                        "error",
                        Json::obj(vec![
                            ("code", Json::num(-32700.0)), // Parse error
                            ("message", Json::str(format!("parse error: {error}"))),
                        ]),
                    ),
                ]);
                write_message(&stdout, &error_response);
                continue;
            }
        };

        let result = server.dispatch(&message);
        for output in &result.outputs {
            write_message(&stdout, output);
        }
        if result.should_exit {
            break;
        }
    }
}

/// Read one `Content-Length` framed message, returning its body.
fn read_message(stdin: &io::Stdin) -> Option<String> {
    let mut reader = stdin.lock();
    let mut header_line = String::new();
    let mut length: Option<usize> = None;

    // Read headers until the blank line that separates them from the body.
    // The body must not be read until the blank terminator has been consumed,
    // otherwise the leading `\r\n` of that blank line ends up in the body.
    loop {
        header_line.clear();
        if reader.read_line(&mut header_line).ok()? == 0 {
            return None; // EOF before the blank separator.
        }
        let trimmed = header_line.trim_end();
        if trimmed.is_empty() {
            break; // Blank line: headers are complete.
        }
        if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
            length = Some(rest.trim().parse().ok()?);
        }
        // Ignore other headers (e.g. Content-Type).
    }

    let mut body = vec![0u8; length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

/// Write one `Content-Length` framed JSON-RPC message.
fn write_message(stdout: &io::Stdout, message: &Json) {
    let body = message.serialize();
    let mut writer = stdout.lock();
    let _ = write!(writer, "Content-Length: {}\r\n\r\n", body.len());
    let _ = writer.write_all(body.as_bytes());
    let _ = writer.flush();
}
