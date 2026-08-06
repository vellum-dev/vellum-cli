use std::fs;
use std::path::Path;

use crate::constants::VELLUM_ROOT;

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

pub fn strip_world_file_pins(packages: &[String]) {
    let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
    let content = match fs::read_to_string(&world_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let new_content: String = content
        .lines()
        .map(|line| {
            for pkg in packages {
                if line.starts_with(&format!("{pkg}=")) {
                    return pkg.clone();
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    let _ = fs::write(&world_path, new_content + "\n");
}

pub fn world_file_members(world_content: &str) -> Vec<String> {
    world_content
        .lines()
        .map(|line| line.split('=').next().unwrap_or(line).trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

pub fn restore_world_file_entries(pinned: &[String], previous_members: &[String]) {
    let (was_member, was_not_member): (Vec<String>, Vec<String>) = pinned
        .iter()
        .cloned()
        .partition(|name| previous_members.iter().any(|m| m == name));

    if !was_member.is_empty() {
        strip_world_file_pins(&was_member);
    }
    if !was_not_member.is_empty() {
        remove_world_file_entries(&was_not_member);
    }
}

pub fn remove_world_file_entries(packages: &[String]) {
    let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
    let content = match fs::read_to_string(&world_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let new_content: String = content
        .lines()
        .filter(|line| {
            !packages.iter().any(|pkg| {
                *line == *pkg || line.starts_with(&format!("{pkg}="))
            })
        })
        .collect::<Vec<_>>()
        .join("\n");

    let _ = fs::write(&world_path, new_content + "\n");
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
    fn world_file_members_strips_pins() {
        let members = world_file_members("foo\nbar=1.2.3-r0\n\nbaz=2.0\n");
        assert_eq!(members, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn world_file_members_empty() {
        assert!(world_file_members("").is_empty());
    }

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
