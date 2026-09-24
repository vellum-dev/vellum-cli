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

const WORLD_CONSTRAINT_OPERATORS: [char; 4] = ['=', '<', '>', '~'];

pub fn world_entry_name(line: &str) -> &str {
    let name_end = line
        .find(|c: char| c == '@' || WORLD_CONSTRAINT_OPERATORS.contains(&c))
        .unwrap_or(line.len());
    &line[..name_end]
}

pub fn world_entry_without_constraint(line: &str) -> &str {
    let constraint_start = line
        .find(WORLD_CONSTRAINT_OPERATORS)
        .unwrap_or(line.len());
    &line[..constraint_start]
}

fn rewrite_world_file(rewrite: impl FnOnce(&str) -> String) {
    let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
    let content = match fs::read_to_string(&world_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = fs::write(&world_path, rewrite(&content) + "\n");
}

fn without_version_pin(line: &str) -> &str {
    let unconstrained = world_entry_without_constraint(line);
    if line[unconstrained.len()..].starts_with('=') {
        unconstrained
    } else {
        line
    }
}

fn strip_pins_from_world(content: &str, packages: &[String]) -> String {
    content
        .lines()
        .map(|line| {
            if packages.iter().any(|pkg| pkg == world_entry_name(line)) {
                without_version_pin(line).to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn remove_entries_from_world(content: &str, packages: &[String]) -> String {
    content
        .lines()
        .filter(|line| !packages.iter().any(|pkg| pkg == world_entry_name(line)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn restore_lines_in_world(content: &str, packages: &[String], snapshot: &str) -> String {
    let mut restored: Vec<&str> = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    for line in content.lines() {
        let name = world_entry_name(line);
        if !packages.iter().any(|pkg| pkg == name) {
            lines.push(line);
        } else if !restored.contains(&name) {
            restored.push(name);
            lines.extend(
                snapshot
                    .lines()
                    .filter(|snapshot_line| world_entry_name(snapshot_line) == name)
                    .map(without_version_pin),
            );
        }
    }
    lines.join("\n")
}

pub fn restore_world_file_lines(packages: &[String], snapshot: &str) {
    rewrite_world_file(|content| restore_lines_in_world(content, packages, snapshot));
}

pub fn strip_world_file_pins(packages: &[String]) {
    rewrite_world_file(|content| strip_pins_from_world(content, packages));
}

pub fn world_file_members(world_content: &str) -> Vec<String> {
    world_content
        .lines()
        .map(|line| world_entry_name(line.trim()).to_string())
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
    rewrite_world_file(|content| remove_entries_from_world(content, packages));
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
    fn world_file_members_strips_tags_and_constraints() {
        let members = world_file_members("tun@testing\nfoo@oxide=1.0-r0\nmicro><Q1abc=\nbar<2\n");
        assert_eq!(members, vec!["tun", "foo", "micro", "bar"]);
    }

    #[test]
    fn world_entry_parts() {
        assert_eq!(world_entry_name("tun@testing=1.0.2-r0"), "tun");
        assert_eq!(world_entry_without_constraint("tun@testing=1.0.2-r0"), "tun@testing");
        assert_eq!(world_entry_name("tun"), "tun");
        assert_eq!(world_entry_without_constraint("tun"), "tun");
        assert_eq!(world_entry_without_constraint("micro><Q1abc="), "micro");
    }

    #[test]
    fn strip_pins_keeps_tag() {
        let packages = vec!["tun".to_string(), "foo".to_string()];
        let world = "tun@testing=1.0.2-r0\nfoo=2.0-r1\nbar=1.0-r0\nmicro><Q1abc=";
        assert_eq!(
            strip_pins_from_world(world, &packages),
            "tun@testing\nfoo\nbar=1.0-r0\nmicro><Q1abc="
        );
    }

    #[test]
    fn strip_pins_leaves_other_constraints() {
        let packages = vec!["foo".to_string()];
        assert_eq!(strip_pins_from_world("foo<2\nfoo-extra=1", &packages), "foo<2\nfoo-extra=1");
    }

    #[test]
    fn restore_puts_back_snapshot_lines() {
        let packages = vec!["foo".to_string(), "tun".to_string(), "new".to_string()];
        let snapshot = "foo@testing<2\ntun@testing\nbar";
        let current = "foo@testing<2\nfoo@testing=1.5-r0\ntun@testing=1.0.2-r0\nbar\nnew=1.0-r0";
        assert_eq!(
            restore_lines_in_world(current, &packages, snapshot),
            "foo@testing<2\ntun@testing\nbar"
        );
    }

    #[test]
    fn restore_strips_version_pins_from_snapshot_lines() {
        let packages = vec!["bar".to_string()];
        assert_eq!(
            restore_lines_in_world("bar=1.4-r0\nfoo", &packages, "bar@testing=1.0-r0\nfoo"),
            "bar@testing\nfoo"
        );
    }

    #[test]
    fn restore_skips_packages_no_longer_in_world() {
        let packages = vec!["replaced".to_string()];
        assert_eq!(restore_lines_in_world("bar", &packages, "replaced\nbar"), "bar");
    }

    #[test]
    fn remove_entries_matches_tagged_and_pinned() {
        let packages = vec!["tun".to_string()];
        let world = "tun@testing=1.0.2-r0\ntun-extra\ntun=1.0.2-r0\nfoo";
        assert_eq!(remove_entries_from_world(world, &packages), "tun-extra\nfoo");
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
