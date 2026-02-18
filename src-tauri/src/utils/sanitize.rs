use std::sync::LazyLock;
use regex::Regex;

static RE_HTML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static RE_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
static RE_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
static RE_LI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<li[^>]*>").unwrap());
static RE_UL_OL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</?(?:ul|ol)[^>]*>").unwrap());
static RE_P_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<p[^>]*>").unwrap());
static RE_P_CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</p>").unwrap());
static RE_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<h[1-4][^>]*>(.*?)</h[1-4]>").unwrap());
static RE_STRONG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<(?:strong|b)>(.*?)</(?:strong|b)>").unwrap());
static RE_EM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<(?:em|i)>(.*?)</(?:em|i)>").unwrap());
static RE_ANCHOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<a\s[^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>"#).unwrap());
static RE_MULTI_BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
static RE_CLOSE_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</(?:li|div|section|article|header|footer|span|code|pre|blockquote|td|tr|th|table|thead|tbody)[^>]*>").unwrap());

const MAX_LEN: usize = 2000;

/// Sanitize HTML release notes into clean Markdown-like text.
///
/// If the input contains no HTML tags, it is returned as-is (assumed Markdown).
pub fn sanitize_release_notes(raw: &str) -> String {
    // Skip processing if input has no HTML tags
    if !RE_HTML_TAG.is_match(raw) {
        let trimmed = raw.trim();
        if trimmed.len() > MAX_LEN {
            return trimmed[..MAX_LEN].to_string();
        }
        return trimmed.to_string();
    }

    let mut s = raw.to_string();

    // 1. Strip XML/HTML comments (including sparkle signatures)
    s = RE_COMMENT.replace_all(&s, "").to_string();

    // 2. Convert <br> to newline
    s = RE_BR.replace_all(&s, "\n").to_string();

    // 3. Convert <li> to "- "
    s = RE_LI.replace_all(&s, "\n- ").to_string();

    // 4. Strip <ul>/<ol> wrappers
    s = RE_UL_OL.replace_all(&s, "").to_string();

    // 5. Convert <p> to double newline
    s = RE_P_OPEN.replace_all(&s, "\n\n").to_string();
    s = RE_P_CLOSE.replace_all(&s, "").to_string();

    // 6. Convert headings to ### prefix
    s = RE_HEADING.replace_all(&s, "\n### $1").to_string();

    // 7. Convert <strong>/<b> to **...**
    s = RE_STRONG.replace_all(&s, "**$1**").to_string();

    // 8. Convert <em>/<i> to *...*
    s = RE_EM.replace_all(&s, "*$1*").to_string();

    // 9. Convert <a href="url">text</a> to [text](url)
    s = RE_ANCHOR.replace_all(&s, "[$2]($1)").to_string();

    // 10. Strip closing tags that add no content
    s = RE_CLOSE_TAGS.replace_all(&s, "").to_string();

    // 11. Strip remaining HTML tags
    s = RE_HTML_TAG.replace_all(&s, "").to_string();

    // 12. Handle semicolon-separated text as bullet points
    if s.contains(';') && !s.contains('\n') {
        let parts: Vec<&str> = s.split(';').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
        if parts.len() > 1 {
            s = parts.iter().map(|p| format!("- {}", p)).collect::<Vec<_>>().join("\n");
        }
    }

    // 13. Decode HTML entities
    s = s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    // 14. Collapse excessive blank lines and trim
    s = RE_MULTI_BLANK.replace_all(&s, "\n\n").to_string();
    s = s.trim().to_string();

    // 15. Truncate to max length
    if s.len() > MAX_LEN {
        s.truncate(MAX_LEN);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_plain_markdown_through() {
        let input = "- Fix bug\n- Add feature";
        assert_eq!(sanitize_release_notes(input), input);
    }

    #[test]
    fn strips_html_comments() {
        let input = "<!-- sparkle:edSignature=abc -->Hello";
        assert_eq!(sanitize_release_notes(input), "Hello");
    }

    #[test]
    fn converts_list_items() {
        let input = "<ul><li>Fix one</li><li>Fix two</li></ul>";
        let result = sanitize_release_notes(input);
        assert!(result.contains("- Fix one"));
        assert!(result.contains("- Fix two"));
    }

    #[test]
    fn converts_bold_and_italic() {
        let input = "<strong>Bold</strong> and <em>italic</em>";
        let result = sanitize_release_notes(input);
        assert!(result.contains("**Bold**"));
        assert!(result.contains("*italic*"));
    }

    #[test]
    fn converts_anchors() {
        let input = r#"<a href="https://example.com">Link</a>"#;
        let result = sanitize_release_notes(input);
        assert!(result.contains("[Link](https://example.com)"));
    }

    #[test]
    fn decodes_html_entities() {
        let input = "<p>A &amp; B &lt; C</p>";
        let result = sanitize_release_notes(input);
        assert!(result.contains("A & B < C"));
    }

    #[test]
    fn truncates_long_input() {
        let long = "x".repeat(3000);
        let result = sanitize_release_notes(&long);
        assert_eq!(result.len(), MAX_LEN);
    }

    #[test]
    fn semicolon_to_bullets() {
        let input = "Fix A; Fix B; Fix C";
        let result = sanitize_release_notes(input);
        assert!(result.contains("- Fix A"));
        assert!(result.contains("- Fix B"));
        assert!(result.contains("- Fix C"));
    }

    #[test]
    fn handles_br_tags() {
        let input = "Line 1<br>Line 2<br/>Line 3";
        let result = sanitize_release_notes(input);
        assert!(result.contains("Line 1\nLine 2\nLine 3"));
    }

    #[test]
    fn converts_headings() {
        let input = "<h2>What's New</h2><p>Stuff</p>";
        let result = sanitize_release_notes(input);
        assert!(result.contains("### What's New"));
    }
}
