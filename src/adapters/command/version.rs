//! Reading and comparing the version a manager reports.

/// Pull a version out of what a `--version` run printed.
///
/// Managers print anything from a bare number to a sentence, so the first
/// token that starts with a digit is taken, with a leading `v` dropped.
pub fn extract(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .filter_map(|token| {
            let token = token.strip_prefix('v').unwrap_or(token);
            let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            trimmed
                .starts_with(|c: char| c.is_ascii_digit())
                .then(|| trimmed.to_string())
        })
        .next()
}

/// Whether `found` is at least `required`, compared segment by segment.
///
/// This is not semver: managers version themselves however they like, so
/// numeric segments compare as numbers and anything else compares as text.
pub fn at_least(found: &str, required: &str) -> bool {
    let split = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect()
    };

    let found = split(found);
    let required = split(required);

    for index in 0..found.len().max(required.len()) {
        let left = found.get(index);
        let right = required.get(index);

        let ordering = match (left, right) {
            (Some(l), Some(r)) => match (l.parse::<u64>(), r.parse::<u64>()) {
                (Ok(l), Ok(r)) => l.cmp(&r),
                _ => l.cmp(r),
            },
            // A version that runs out of segments is the older of the two.
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (None, None) => std::cmp::Ordering::Equal,
        };

        match ordering {
            std::cmp::Ordering::Equal => continue,
            other => return other == std::cmp::Ordering::Greater,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_found_beside_a_name() {
        assert_eq!(extract("soar-cli 0.12.7").as_deref(), Some("0.12.7"));
        assert_eq!(extract("demo v1.2.3\n").as_deref(), Some("1.2.3"));
        assert_eq!(extract("5.9-1").as_deref(), Some("5.9-1"));
        assert_eq!(extract("no version here"), None);
    }

    #[test]
    fn versions_compare_by_segment() {
        assert!(at_least("0.13.0", "0.13.0"));
        assert!(at_least("0.13.1", "0.13.0"));
        assert!(at_least("1.0.0", "0.99.0"));
        assert!(!at_least("0.12.7", "0.13.0"));
        assert!(!at_least("0.9.0", "0.10.0"));
    }

    #[test]
    fn a_longer_version_outranks_the_prefix_it_extends() {
        assert!(at_least("0.13.0.1", "0.13.0"));
        assert!(!at_least("0.13", "0.13.1"));
    }
}
