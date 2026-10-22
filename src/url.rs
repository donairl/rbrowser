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

/// Domains from `$XDG_CONFIG_HOME/rbrowser/blocklist.txt` (or `~/.config/...`),
/// one per line, `#` for comments. Missing file means nothing is blocked.
pub fn load_blocklist() -> Vec<String> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")));
    let Some(path) = dir.map(|d| d.join("rbrowser/blocklist.txt")) else {
        return Vec::new();
    };
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().trim_start_matches("www.").to_lowercase())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// True if the URL's host is a blocked domain or one of its subdomains.
pub fn is_blocked(url: &str, blocklist: &[String]) -> bool {
    let Some(host) = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_lowercase))
    else {
        return false;
    };
    blocklist
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocklist() {
        let list = vec!["youtube.com".to_string(), "x.com".to_string()];
        assert!(is_blocked("https://youtube.com/watch?v=1", &list));
        assert!(is_blocked("https://www.youtube.com", &list));
        assert!(is_blocked("https://m.YouTube.com", &list));
        assert!(!is_blocked("https://notyoutube.com", &list));
        assert!(!is_blocked("https://box.com", &list));
        assert!(!is_blocked("about:blank", &list));
        assert!(!is_blocked("focus://localhost/index.html", &list));
    }

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
