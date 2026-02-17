use std::cmp::Ordering;

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
        if ch != '.' && ch != '-' && ch != ' ' && ch != '(' && ch != ')' {
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
}
