use std::cmp::Ordering;

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }

    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    let min_len = a_parts.len().min(b_parts.len());

    for i in 0..min_len {
        let a_num: i32 = a_parts[i]
            .split('-')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let b_num: i32 = b_parts[i]
            .split('-')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        match a_num.cmp(&b_num) {
            Ordering::Greater => return Ordering::Greater,
            Ordering::Less => return Ordering::Less,
            Ordering::Equal => continue,
        }
    }

    a_parts.len().cmp(&b_parts.len())
}

pub fn version_gte(a: &str, b: &str) -> bool {
    compare_versions(a, b) != Ordering::Less
}

pub fn version_lt(a: &str, b: &str) -> bool {
    compare_versions(a, b) == Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_equal_versions() {
        assert_eq!(compare_versions("3.10.0.0", "3.10.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1.0"), Ordering::Equal);
        assert_eq!(compare_versions("0", "0"), Ordering::Equal);
    }

    #[test]
    fn compare_greater_versions() {
        assert_eq!(compare_versions("3.10.0.0", "3.9.0.0"), Ordering::Greater);
        assert_eq!(compare_versions("2.0", "1.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.10", "1.9"), Ordering::Greater);
    }

    #[test]
    fn compare_lesser_versions() {
        assert_eq!(compare_versions("3.9.0.0", "3.10.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.9", "2.0"), Ordering::Less);
        assert_eq!(compare_versions("1.9", "1.10"), Ordering::Less);
    }

    #[test]
    fn compare_different_lengths() {
        assert_eq!(compare_versions("3", "3.0"), Ordering::Less);
        assert_eq!(compare_versions("3.0", "3"), Ordering::Greater);
        assert_eq!(compare_versions("3.0.0", "3.0"), Ordering::Greater);
        assert_eq!(compare_versions("3.0", "3.0.0"), Ordering::Less);
    }

    #[test]
    fn compare_with_prerelease() {
        assert_eq!(compare_versions("3.10.0-r1", "3.10.0"), Ordering::Equal);
        assert_eq!(compare_versions("3.10.0-r2", "3.10.0-r1"), Ordering::Equal);
        assert_eq!(compare_versions("3.10.0-r1", "3.9.0"), Ordering::Greater);
    }

    #[test]
    fn compare_empty_strings() {
        assert_eq!(compare_versions("", ""), Ordering::Equal);
        assert_eq!(compare_versions("1", ""), Ordering::Greater);
        assert_eq!(compare_versions("", "1"), Ordering::Less);
    }

    #[test]
    fn compare_single_component() {
        assert_eq!(compare_versions("1", "2"), Ordering::Less);
        assert_eq!(compare_versions("2", "1"), Ordering::Greater);
        assert_eq!(compare_versions("10", "9"), Ordering::Greater);
    }

    #[test]
    fn version_gte_returns_true_when_greater_or_equal() {
        assert!(version_gte("3.10.0.0", "3.10.0.0"));
        assert!(version_gte("3.10.0.0", "3.9.0.0"));
        assert!(version_gte("4.0", "3.99"));
    }

    #[test]
    fn version_gte_returns_false_when_less() {
        assert!(!version_gte("3.9.0.0", "3.10.0.0"));
        assert!(!version_gte("2.0", "3.0"));
    }

    #[test]
    fn version_lt_returns_true_when_less() {
        assert!(version_lt("3.9.0.0", "3.10.0.0"));
        assert!(version_lt("2.0", "3.0"));
    }

    #[test]
    fn version_lt_returns_false_when_greater_or_equal() {
        assert!(!version_lt("3.10.0.0", "3.10.0.0"));
        assert!(!version_lt("3.10.0.0", "3.9.0.0"));
    }
}
