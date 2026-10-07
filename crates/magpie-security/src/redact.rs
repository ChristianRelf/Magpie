use once_cell::sync::Lazy;
use regex::Regex;

static PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    [
        // Anthropic, OpenAI, OpenRouter, DeepSeek style keys.
        (r"sk-(?:ant-|proj-|or-v1-|svcacct-)?[A-Za-z0-9_\-]{16,}", "sk-[REDACTED]"),
        // Google API keys.
        (r"AIza[0-9A-Za-z_\-]{30,}", "AIza[REDACTED]"),
        // Groq keys.
        (r"gsk_[A-Za-z0-9]{20,}", "gsk_[REDACTED]"),
        // Magpie client keys.
        (r"mgp_[A-Za-z0-9_\-]{16,}", "mgp_[REDACTED]"),
        // Bearer tokens in headers or logs.
        (r"(?i)(bearer\s+)[A-Za-z0-9._\-~+/]{12,}=*", "${1}[REDACTED]"),
        // Header/query/json key-value secrets.
        (
            r#"(?i)((?:x-api-key|x-goog-api-key|api[_-]?key|access[_-]?token|refresh[_-]?token|authorization|password|secret)["']?\s*[:=]\s*["']?)[^\s"'&,}]{6,}"#,
            "${1}[REDACTED]",
        ),
        // JWT-shaped tokens.
        (r"eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}", "[REDACTED_JWT]"),
    ]
    .into_iter()
    .map(|(p, r)| (Regex::new(p).expect("valid redaction regex"), r))
    .collect()
});

/// Remove anything that looks like a credential from `input`. Applied to all
/// provider error messages, logs and diagnostic output.
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    for (re, replacement) in PATTERNS.iter() {
        if re.is_match(&out) {
            out = re.replace_all(&out, *replacement).into_owned();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_known_key_shapes() {
        let s = redact("key sk-ant-api03-abcdefghijklmnopqrstuvwxyz and AIzaSyA1234567890abcdefghijklmnopqrstu");
        assert!(!s.contains("abcdefghijklmnop"), "{s}");
        assert!(!s.contains("SyA1234567890"), "{s}");
    }

    #[test]
    fn redacts_headers_and_bearer() {
        let s = redact("Authorization: Bearer abcdef1234567890xyz");
        assert!(!s.contains("abcdef1234567890xyz"), "{s}");
        let s = redact(r#"{"api_key": "supersecretvalue"}"#);
        assert!(!s.contains("supersecretvalue"), "{s}");
        let s = redact("https://x/y?key=AIzaSomething12345678901234567890123&alt=sse");
        assert!(!s.contains("Something123"), "{s}");
    }

    #[test]
    fn leaves_ordinary_text() {
        assert_eq!(redact("rate limit exceeded for model"), "rate limit exceeded for model");
    }
}
