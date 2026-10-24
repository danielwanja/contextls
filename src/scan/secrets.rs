//! Cheap "this looks like a secret" check. Context files get pasted into
//! prompts, so a key here leaks to a model provider.

use std::sync::LazyLock;

use regex::Regex;

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("OpenAI/Anthropic-style key", r"\bsk-[A-Za-z0-9_-]{20,}"),
        ("AWS access key", r"\bAKIA[0-9A-Z]{16}\b"),
        ("GitHub token", r"\bgh[pousr]_[A-Za-z0-9]{36,}\b"),
        ("Slack token", r"\bxox[baprs]-[A-Za-z0-9-]{10,}"),
        ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
        ("private key block", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
        (
            "credential assignment",
            r#"(?i)\b(api[_-]?key|secret|token|password|passwd)\b\s*[:=]\s*["']?[A-Za-z0-9_\-/+=]{16,}"#,
        ),
    ]
    .into_iter()
    .map(|(name, re)| (name, Regex::new(re).unwrap()))
    .collect()
});

/// One warning per matching line.
pub fn scan(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        for (name, re) in PATTERNS.iter() {
            if re.is_match(line) {
                out.push(format!("possible secret ({name}) on line {}", i + 1));
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn flags_keys_not_prose() {
        let w = super::scan(
            "api_key = \"abcdefghijklmnopqrstuvwxyz\"\nUse the API key from the vault.\n",
        );
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("line 1"));
    }
}
