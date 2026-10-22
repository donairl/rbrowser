use url::form_urlencoded;

pub fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "about:blank".to_string();
    }

    // Already explicit protocols
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("about:")
        || trimmed.starts_with("file://")
        || trimmed.starts_with("data:")
    {
        return trimmed.to_string();
    }

    // Local hostnames without scheme
    if trimmed.starts_with("localhost") || trimmed.starts_with("127.0.0.1") {
        return format!("http://{trimmed}");
    }

    // Single token check: if it contains no spaces and has a dot with a valid TLD/host structure
    if !trimmed.contains(char::is_whitespace) {
        let first_slash = trimmed.find('/').unwrap_or(trimmed.len());
        let host_candidate = &trimmed[..first_slash];

        // If host contains dot and has non-empty domain parts, treat as domain
        if let Some(dot_pos) = host_candidate.rfind('.') {
            let tld = &host_candidate[dot_pos + 1..];
            // Split off port if present
            let tld_clean = tld.split(':').next().unwrap_or(tld);
            if !tld_clean.is_empty()
                && (tld_clean.chars().all(|c| c.is_alphabetic())
                    || tld_clean.chars().all(|c| c.is_numeric()))
            {
                return format!("https://{trimmed}");
            }
        }
    }

    // Fallback: search query on DuckDuckGo
    let encoded_query = form_urlencoded::byte_serialize(trimmed.as_bytes()).collect::<String>();
    format!("https://duckduckgo.com/?q={encoded_query}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(normalize_url(""), "about:blank");
        assert_eq!(normalize_url("   "), "about:blank");
    }

    #[test]
    fn test_explicit_scheme() {
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
        assert_eq!(
            normalize_url("http://example.com/page"),
            "http://example.com/page"
        );
        assert_eq!(normalize_url("about:blank"), "about:blank");
        assert_eq!(
            normalize_url("file:///tmp/test.html"),
            "file:///tmp/test.html"
        );
    }

    #[test]
    fn test_localhost_and_ips() {
        assert_eq!(normalize_url("localhost:3000"), "http://localhost:3000");
        assert_eq!(normalize_url("localhost"), "http://localhost");
        assert_eq!(normalize_url("127.0.0.1:8080"), "http://127.0.0.1:8080");
    }

    #[test]
    fn test_domain_names() {
        assert_eq!(normalize_url("github.com"), "https://github.com");
        assert_eq!(
            normalize_url("crates.io/crates/wry"),
            "https://crates.io/crates/wry"
        );
        assert_eq!(
            normalize_url("en.wikipedia.org"),
            "https://en.wikipedia.org"
        );
    }

    #[test]
    fn test_search_queries() {
        assert_eq!(
            normalize_url("how to use rust wry"),
            "https://duckduckgo.com/?q=how+to+use+rust+wry"
        );
        assert_eq!(
            normalize_url("rust programming"),
            "https://duckduckgo.com/?q=rust+programming"
        );
    }
}
