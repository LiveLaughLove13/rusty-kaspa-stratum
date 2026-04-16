//! Line-oriented framing for Stratum over TCP: strip NULs, accumulate bytes, split on `\n`.
//! Extracted for unit tests without spinning a full listener.

/// Remove embedded NUL bytes (some firmware sends them between messages).
pub fn strip_nul_bytes(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().copied().filter(|&b| b != 0).collect()
}

/// Append UTF-8 lossy chunk to `buffer`, then drain complete `\n`-terminated lines.
/// Each line is trimmed; empty lines after trim are skipped (same as `stratum_listener`).
pub fn push_lossy_and_drain_lines(buffer: &mut String, chunk: &str) -> Vec<String> {
    buffer.push_str(chunk);
    let mut out = Vec::new();
    while let Some(pos) = buffer.find('\n') {
        let line = buffer[..pos].trim().to_string();
        *buffer = buffer[pos + 1..].to_string();
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

/// True if a trimmed line looks like HTTP/1.x or HTTP/2 connection preface (not Stratum JSON-RPC).
pub fn line_looks_like_http(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("PRI * HTTP/2.0")
        || t.starts_with("PRI * HTTP/2")
        || t == "SM"
        || t.starts_with("GET ")
        || t.starts_with("POST ")
        || t.starts_with("PUT ")
        || t.starts_with("DELETE ")
        || t.starts_with("HEAD ")
        || t.starts_with("OPTIONS ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_nul_preserves_payload() {
        let raw = b"{\"id\":1}\0\n";
        assert_eq!(strip_nul_bytes(raw), b"{\"id\":1}\n");
    }

    #[test]
    fn drain_two_lines_partial_remainder() {
        let mut buf = String::new();
        let a = push_lossy_and_drain_lines(&mut buf, "{\"a\":1}\n{\"b\":");
        assert_eq!(a, vec!["{\"a\":1}".to_string()]);
        assert_eq!(buf, "{\"b\":");
        let b = push_lossy_and_drain_lines(&mut buf, "2}\n");
        assert_eq!(b, vec!["{\"b\":2}".to_string()]);
        assert!(buf.is_empty());
    }

    #[test]
    fn empty_and_whitespace_lines_skipped() {
        let mut buf = String::new();
        let v = push_lossy_and_drain_lines(&mut buf, "  \n  foo  \n\n");
        assert_eq!(v, vec!["foo".to_string()]);
    }

    #[test]
    fn http_detection() {
        assert!(line_looks_like_http("GET / HTTP/1.1"));
        assert!(line_looks_like_http("PRI * HTTP/2.0\r"));
        assert!(!line_looks_like_http("{\"jsonrpc\":\"2.0\",\"method\":\"mining.subscribe\"}"));
    }
}
