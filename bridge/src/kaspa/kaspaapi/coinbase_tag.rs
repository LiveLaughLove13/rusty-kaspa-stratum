const STRATUM_COINBASE_TAG_BYTES: &[u8] = b"RK-Stratum";
const MAX_COINBASE_TAG_SUFFIX_LEN: usize = 64;

fn sanitize_coinbase_tag_suffix(suffix: &str) -> Option<String> {
    let suffix = suffix.trim().trim_start_matches('/');
    if suffix.is_empty() {
        return None;
    }

    let mut out = String::with_capacity(suffix.len().min(MAX_COINBASE_TAG_SUFFIX_LEN));
    for ch in suffix.chars() {
        if out.len() >= MAX_COINBASE_TAG_SUFFIX_LEN {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
        } else if ch.is_ascii_whitespace() {
            out.push('_');
        }
    }

    let out = out.trim_matches('_').to_string();
    if out.is_empty() { None } else { Some(out) }
}

pub(super) fn build_coinbase_tag_bytes(suffix: Option<&str>) -> Vec<u8> {
    let mut tag = STRATUM_COINBASE_TAG_BYTES.to_vec();
    if let Some(suffix) = suffix.and_then(sanitize_coinbase_tag_suffix) {
        tag.push(b'/');
        tag.extend_from_slice(suffix.as_bytes());
    }
    tag
}
