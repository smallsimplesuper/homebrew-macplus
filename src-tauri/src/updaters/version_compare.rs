use std::cmp::Ordering;

/// Strip Homebrew cask version tokens (comma-separated hash/qualifier).
/// e.g. "1.1.3363,ee424797ca4d37a06f6b4a1e48dc944838ac3b18" → "1.1.3363"
pub fn strip_brew_version_token(version: &str) -> &str {
    version.split(',').next().unwrap_or(version)
}

/// Compare two version strings flexibly.
/// Returns Ordering::Greater if `available` is newer than `current`.
pub fn is_newer(current: &str, available: &str) -> bool {
    flexible_compare(current, available) == Ordering::Less
}

pub fn flexible_compare(a: &str, b: &str) -> Ordering {
    // Try semver first
    if let (Ok(va), Ok(vb)) = (semver::Version::parse(a), semver::Version::parse(b)) {
        return va.cmp(&vb);
    }

    // Component-by-component numeric comparison
    let seg_a = split_segments(a);
    let seg_b = split_segments(b);

    let max_len = seg_a.len().max(seg_b.len());
    for i in 0..max_len {
        let sa = seg_a.get(i).map(|s| s.as_str()).unwrap_or("0");
        let sb = seg_b.get(i).map(|s| s.as_str()).unwrap_or("0");

        match (sa.parse::<u64>(), sb.parse::<u64>()) {
            (Ok(na), Ok(nb)) => match na.cmp(&nb) {
                Ordering::Equal => continue,
                other => return other,
            },
            _ => match sa.cmp(sb) {
                Ordering::Equal => continue,
                other => return other,
            },
        }
    }

    Ordering::Equal
}

fn split_segments(version: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut was_digit = false;

    for ch in version.chars() {
        let is_digit = ch.is_ascii_digit();
        if !current.is_empty() && is_digit != was_digit {
            segments.push(current.clone());
            current.clear();
        }
        if ch != '.' && ch != '-' && ch != ' ' && ch != '(' && ch != ')' && ch != ',' {
            current.push(ch);
        } else if !current.is_empty() {
            segments.push(current.clone());
            current.clear();
        }
        was_digit = is_digit;
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver() {
        assert!(is_newer("1.0.0", "1.0.1"));
        assert!(is_newer("1.0.0", "2.0.0"));
        assert!(!is_newer("2.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_dotted() {
        assert!(is_newer("17.2", "17.3"));
        assert!(is_newer("1.2.3", "1.2.4"));
        assert!(!is_newer("2.0", "1.9"));
    }

    #[test]
    fn test_different_lengths() {
        assert!(is_newer("1.0", "1.0.1"));
        assert!(is_newer("5", "5.1"));
    }

    #[test]
    fn test_strip_brew_version_token() {
        assert_eq!(strip_brew_version_token("1.1.3363,ee424797ca4d37a06f6b4a1e48dc944838ac3b18"), "1.1.3363");
        assert_eq!(strip_brew_version_token("4.0.2"), "4.0.2");
        assert_eq!(strip_brew_version_token(""), "");
        assert_eq!(strip_brew_version_token("latest"), "latest");
    }

    #[test]
    fn test_stripped_brew_version_comparison() {
        // After strip_brew_version_token, comparison should work correctly
        let raw = "1.1.3363,ee424797ca4d37a06f6b4a1e48dc944838ac3b18";
        let stripped = strip_brew_version_token(raw);
        assert!(!is_newer("1.1.3363", stripped));
        assert!(is_newer("1.1.3362", stripped));
        assert!(!is_newer("1.1.3364", stripped));
    }

    #[test]
    fn test_comma_in_split_segments() {
        // Commas should be treated as separators (like dots).
        // The hash part also splits on digit/non-digit boundaries.
        let segs = split_segments("1.1.3363,ee424797");
        assert_eq!(segs, vec!["1", "1", "3363", "ee", "424797"]);

        // Without strip_brew_version_token, the extra hash segments make it
        // appear "newer" — that's why we always strip before comparing.
        assert!(is_newer("1.1.3362", "1.1.3363,ee424797"));
    }
}
