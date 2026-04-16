//! Embedded and on-disk static assets for the web dashboard (`/`, page HTML, `/static/...`).

pub(crate) fn content_type_for_path(path: &str) -> &'static str {
    let p = path.to_ascii_lowercase();
    if p.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if p.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if p.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if p.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

pub(crate) fn try_read_static_file(url_path: &str) -> Option<(String, Vec<u8>)> {
    // Files are vendored under bridge/static.
    // URL layout expected by the dashboard:
    // - / -> index.html
    // - /trends.html, /blocks.html, /workers.html, /node.html (dashboard sections)
    // - /raw.html
    // - /static/... -> maps to bridge/static/... (strip leading /static/)
    let rel = match url_path {
        "/" => "index.html".to_string(),
        "/index.html" => "index.html".to_string(),
        "/trends.html" => "trends.html".to_string(),
        "/blocks.html" => "blocks.html".to_string(),
        "/workers.html" => "workers.html".to_string(),
        "/node.html" => "node.html".to_string(),
        "/raw.html" => "raw.html".to_string(),
        p if p.starts_with("/static/") => p.trim_start_matches("/static/").to_string(),
        _ => return None,
    };

    // Prevent path traversal
    if rel.contains("..") || rel.contains('\\') {
        return None;
    }

    // Prefer embedded assets for production/portable binaries.
    // Fall back to reading from disk to keep local development simple.
    static STATIC_DIR: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/static");

    if let Some(f) = STATIC_DIR.get_file(&rel) {
        return Some((rel, f.contents().to_vec()));
    }

    let file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("static")
        .join(&rel);
    let bytes = std::fs::read(&file_path).ok()?;
    Some((rel, bytes))
}
