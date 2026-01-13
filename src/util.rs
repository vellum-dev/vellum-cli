use std::fs;
use std::path::Path;

pub fn remove_glob(pattern: &str) {
    let dir = Path::new(pattern).parent().unwrap_or(Path::new("."));
    let file_pattern = Path::new(pattern)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if matches_glob(name, file_pattern) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

pub fn matches_glob(name: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("*.apk") {
        name.starts_with(prefix) && name.ends_with(".apk")
    } else {
        name == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_glob_exact_match() {
        assert!(matches_glob("foo.apk", "foo.apk"));
        assert!(matches_glob("test-package.apk", "test-package.apk"));
    }

    #[test]
    fn matches_glob_exact_no_match() {
        assert!(!matches_glob("foo.apk", "bar.apk"));
        assert!(!matches_glob("foo.txt", "foo.apk"));
    }

    #[test]
    fn matches_glob_wildcard_match() {
        assert!(matches_glob("remarkable-os-3.10.0.0.apk", "remarkable-os-*.apk"));
        assert!(matches_glob("remarkable-os-1.0.apk", "remarkable-os-*.apk"));
        assert!(matches_glob("prefix-anything.apk", "prefix-*.apk"));
    }

    #[test]
    fn matches_glob_wildcard_no_match() {
        assert!(!matches_glob("other-package.apk", "remarkable-os-*.apk"));
        assert!(!matches_glob("remarkable-os-3.10.txt", "remarkable-os-*.apk"));
        assert!(!matches_glob("remarkable-os-3.10.apk.bak", "remarkable-os-*.apk"));
    }

    #[test]
    fn matches_glob_empty_prefix() {
        assert!(matches_glob("anything.apk", "*.apk"));
        assert!(matches_glob(".apk", "*.apk"));
    }
}
