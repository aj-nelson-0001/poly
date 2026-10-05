//! Poly language server binary.
//!
//! Speaks JSON-RPC 2.0 over stdio with `Content-Length` framing, the transport
//! convention used by most editors.  The server itself lives in the `server`
//! module so it can be embedded and unit-tested without a pipe.

// `read_exact` resolves through `BufRead`, so `Read` is not needed here.
use std::io::{self, BufRead, Write};

use poly_lsp::json::{self, Json};
use poly_lsp::server::Server;

/// Largest `Content-Length` frame accepted, in bytes.
///
/// The header is client-controlled, so trusting it directly let a single frame
/// ask for an arbitrary allocation: `Content-Length: 100000000000` aborted the
/// process with an out-of-memory abort before a single byte was read. Capping
/// the header keeps the allocation proportional to traffic a language server
/// actually sees. Real frames -- even for large generated files and big test
/// suites -- sit well under 8 MiB.
///
/// The figure above is measured, not guessed: driving a real session
/// (initialize, didOpen, documentSymbol, workspace/symbol) against the largest
/// `.poly` file in this repo (tetris.poly, 54 KB) peaks at a 21 KB frame, so
/// the cap carries roughly 400x headroom. It is deliberately generous: the job
/// is to refuse an absurd header, not to second-guess a large genuine document.
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

/// The server main loop: read a framed message, dispatch it, write the
/// responses, and stop when the client exits or the pipe closes.
fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server::new();

    // Read Content-Length framed messages until EOF (client closed the pipe)
    // or the server dispatches an `exit` notification.
    while let Some(content) = read_message(&mut stdin.lock()) {
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

        // Dispatch to the server state machine and stream back everything it
        // produced (responses and diagnostics notifications alike).
        let result = server.dispatch(&message);
        for output in &result.outputs {
            write_message(&stdout, output);
        }
        // `exit` (or a shutdown-followed-by-exit sequence) ends the loop.
        if result.should_exit {
            break;
        }
    }
}

/// Read one `Content-Length` framed message, returning its body.
///
/// Generic over the reader so the framing and its size limit can be tested
/// against an in-memory buffer; production passes a locked stdin.
fn read_message<R: BufRead>(reader: &mut R) -> Option<String> {
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
            let declared: usize = rest.trim().parse().ok()?;
            // Reject an oversized frame before allocating for it. Returning
            // `None` ends the session: the stream no longer has a usable
            // framing, so resynchronizing would risk reading body bytes as a
            // header. The header value is attacker-controlled, so it is
            // checked before it ever reaches an allocation.
            if declared > MAX_MESSAGE_BYTES {
                eprintln!(
                    "refusing frame of {declared} bytes: over the {MAX_MESSAGE_BYTES} byte limit"
                );
                return None;
            }
            length = Some(declared);
        }
        // Ignore other headers (e.g. Content-Type).
    }

    // The length has already been bounded by `MAX_MESSAGE_BYTES` above, so
    // this allocation is proportional to traffic the server accepts rather
    // than to whatever the client asked for.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    /// Frame `body` the way an LSP client does.
    fn frame(body: &str) -> String {
        format!("Content-Length: {}\r\n\r\n{body}", body.len())
    }

    #[test]
    fn reads_a_normal_message() -> Result<(), Box<dyn std::error::Error>> {
        let framed = frame("{\"jsonrpc\":\"2.0\"}");
        let mut reader = BufReader::new(framed.as_bytes());
        let message = read_message(&mut reader);
        assert_eq!(message.as_deref(), Some("{\"jsonrpc\":\"2.0\"}"));
        Ok(())
    }

    #[test]
    fn reads_consecutive_messages() -> Result<(), Box<dyn std::error::Error>> {
        // The reader is reused across messages, so the first frame must not
        // consume bytes belonging to the second.
        let mut stream = String::new();
        stream.push_str(&frame("{\"id\":1}"));
        stream.push_str(&frame("{\"id\":2}"));
        let mut reader = BufReader::new(stream.as_bytes());
        assert_eq!(read_message(&mut reader).as_deref(), Some("{\"id\":1}"));
        assert_eq!(read_message(&mut reader).as_deref(), Some("{\"id\":2}"));
        assert_eq!(read_message(&mut reader), None, "stream should be drained");
        Ok(())
    }

    #[test]
    fn oversized_content_length_is_refused_without_allocating() {
        // Regression: the header was trusted directly, so
        // `Content-Length: 100000000000` aborted the process with an
        // out-of-memory abort before a byte was read. The check has to happen
        // before the allocation, so the frame is refused outright.
        let mut reader = BufReader::new(&b"Content-Length: 100000000000\r\n\r\n"[..]);
        assert_eq!(read_message(&mut reader), None);
    }

    #[test]
    fn usize_max_content_length_is_refused() {
        // This value overflows the allocation's capacity computation, which
        // used to abort as an uncontrolled panic rather than a clean refusal.
        let mut reader = BufReader::new(&b"Content-Length: 18446744073709551615\r\n\r\n"[..]);
        assert_eq!(read_message(&mut reader), None);
    }

    #[test]
    fn a_large_but_permitted_frame_is_still_read() -> Result<(), Box<dyn std::error::Error>> {
        // The cap must not reject real traffic: a multi-megabyte document
        // (a generated file, a big test suite) has to come through intact.
        let body = "x".repeat(2 * 1024 * 1024);
        let framed = frame(&body);
        let mut reader = BufReader::new(framed.as_bytes());
        match read_message(&mut reader) {
            Some(read) => assert_eq!(read.len(), body.len()),
            None => panic!("a 2 MiB frame is within the limit and must be read"),
        }
        Ok(())
    }

    #[test]
    fn unparseable_length_ends_the_stream() -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(&b"Content-Length: abc\r\n\r\n"[..]);
        assert_eq!(read_message(&mut reader), None);
        Ok(())
    }
}
