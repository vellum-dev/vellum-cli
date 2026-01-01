use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};

use anyhow::{anyhow, Result};
use flate2::bufread::MultiGzDecoder;
use tar::Archive;

use super::version::{version_gte, version_lt};

#[derive(Debug, Clone, Default)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub depends: Vec<String>,
}

impl Package {
    pub fn get_os_constraints(&self) -> (Option<String>, Option<String>) {
        let mut min_ver = None;
        let mut max_ver = None;

        for dep in &self.depends {
            if let Some(v) = dep.strip_prefix("remarkable-os>=") {
                min_ver = Some(v.to_string());
            } else if let Some(v) = dep.strip_prefix("remarkable-os<") {
                max_ver = Some(v.to_string());
            }
        }

        (min_ver, max_ver)
    }

    pub fn is_compatible_with_os(&self, os_version: &str) -> bool {
        let (min_ver, max_ver) = self.get_os_constraints();

        if min_ver.is_none() && max_ver.is_none() {
            return true;
        }

        if let Some(ref min) = min_ver {
            if !version_gte(os_version, min) {
                return false;
            }
        }

        if let Some(ref max) = max_ver {
            if !version_lt(os_version, max) {
                return false;
            }
        }

        true
    }
}

pub fn parse_index_tar_gz(path: &str) -> Result<Vec<Package>> {
    let f = File::open(path)?;
    parse_index_from_tar_gz(f)
}

pub fn fetch_remote_index(repo_url: &str, arch: &str) -> Result<Vec<Package>> {
    let url = format!("{}/{}/APKINDEX.tar.gz", repo_url.trim_end_matches('/'), arch);

    let resp = ureq::get(&url).call().map_err(|e| anyhow!("HTTP request failed: {e}"))?;

    if resp.status() != 200 {
        return Err(anyhow!("HTTP {}", resp.status()));
    }

    let mut data = Vec::new();
    resp.into_reader().read_to_end(&mut data)?;

    parse_index_from_tar_gz(Cursor::new(data))
}

fn parse_index_from_tar_gz<R: Read>(reader: R) -> Result<Vec<Package>> {
    let mut data = Vec::new();
    let mut reader = reader;
    reader.read_to_end(&mut data)?;

    // Alpine's APKINDEX.tar.gz consists of multiple concatenated gzip streams:
    // 1. Signature segment (first gzip stream)
    // 2. Index tarball with DESCRIPTION and APKINDEX (second gzip stream)
    // MultiGzDecoder handles concatenated streams, unlike GzDecoder which stops after the first.
    let gz = MultiGzDecoder::new(Cursor::new(data));
    let mut archive = Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.to_string_lossy() == "APKINDEX" {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            return parse_apkindex(BufReader::new(content.as_bytes()));
        }
    }

    Err(anyhow!("APKINDEX not found in archive"))
}

fn parse_apkindex<R: BufRead>(reader: R) -> Result<Vec<Package>> {
    let mut packages = Vec::new();
    let mut current = Package::default();

    for line in reader.lines() {
        let line = line?;

        if line.is_empty() {
            if !current.name.is_empty() {
                packages.push(current);
            }
            current = Package::default();
            continue;
        }

        if line.len() < 2 || line.as_bytes()[1] != b':' {
            continue;
        }

        let key = line.as_bytes()[0];
        let val = &line[2..];

        match key {
            b'P' => current.name = val.to_string(),
            b'V' => current.version = val.to_string(),
            b'D' => current.depends = val.split_whitespace().map(|s| s.to_string()).collect(),
            _ => {}
        }
    }

    if !current.name.is_empty() {
        packages.push(current);
    }

    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn make_package(name: &str, version: &str, depends: Vec<&str>) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
            depends: depends.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn get_os_constraints_with_min_only() {
        let pkg = make_package("test", "1.0", vec!["remarkable-os>=3.10.0.0"]);
        let (min, max) = pkg.get_os_constraints();
        assert_eq!(min, Some("3.10.0.0".to_string()));
        assert_eq!(max, None);
    }

    #[test]
    fn get_os_constraints_with_max_only() {
        let pkg = make_package("test", "1.0", vec!["remarkable-os<4.0.0.0"]);
        let (min, max) = pkg.get_os_constraints();
        assert_eq!(min, None);
        assert_eq!(max, Some("4.0.0.0".to_string()));
    }

    #[test]
    fn get_os_constraints_with_both() {
        let pkg = make_package("test", "1.0", vec![
            "remarkable-os>=3.10.0.0",
            "remarkable-os<4.0.0.0",
        ]);
        let (min, max) = pkg.get_os_constraints();
        assert_eq!(min, Some("3.10.0.0".to_string()));
        assert_eq!(max, Some("4.0.0.0".to_string()));
    }

    #[test]
    fn get_os_constraints_with_none() {
        let pkg = make_package("test", "1.0", vec!["other-dep"]);
        let (min, max) = pkg.get_os_constraints();
        assert_eq!(min, None);
        assert_eq!(max, None);
    }

    #[test]
    fn get_os_constraints_empty_deps() {
        let pkg = make_package("test", "1.0", vec![]);
        let (min, max) = pkg.get_os_constraints();
        assert_eq!(min, None);
        assert_eq!(max, None);
    }

    #[test]
    fn is_compatible_no_constraints() {
        let pkg = make_package("test", "1.0", vec![]);
        assert!(pkg.is_compatible_with_os("3.10.0.0"));
        assert!(pkg.is_compatible_with_os("1.0.0.0"));
    }

    #[test]
    fn is_compatible_min_constraint_satisfied() {
        let pkg = make_package("test", "1.0", vec!["remarkable-os>=3.10.0.0"]);
        assert!(pkg.is_compatible_with_os("3.10.0.0"));
        assert!(pkg.is_compatible_with_os("3.11.0.0"));
        assert!(pkg.is_compatible_with_os("4.0.0.0"));
    }

    #[test]
    fn is_compatible_min_constraint_not_satisfied() {
        let pkg = make_package("test", "1.0", vec!["remarkable-os>=3.10.0.0"]);
        assert!(!pkg.is_compatible_with_os("3.9.0.0"));
        assert!(!pkg.is_compatible_with_os("2.0.0.0"));
    }

    #[test]
    fn is_compatible_max_constraint_satisfied() {
        let pkg = make_package("test", "1.0", vec!["remarkable-os<4.0.0.0"]);
        assert!(pkg.is_compatible_with_os("3.10.0.0"));
        assert!(pkg.is_compatible_with_os("3.99.99.99"));
    }

    #[test]
    fn is_compatible_max_constraint_not_satisfied() {
        let pkg = make_package("test", "1.0", vec!["remarkable-os<4.0.0.0"]);
        assert!(!pkg.is_compatible_with_os("4.0.0.0"));
        assert!(!pkg.is_compatible_with_os("5.0.0.0"));
    }

    #[test]
    fn is_compatible_both_constraints_in_range() {
        let pkg = make_package("test", "1.0", vec![
            "remarkable-os>=3.10.0.0",
            "remarkable-os<4.0.0.0",
        ]);
        assert!(pkg.is_compatible_with_os("3.10.0.0"));
        assert!(pkg.is_compatible_with_os("3.15.0.0"));
        assert!(pkg.is_compatible_with_os("3.99.99.99"));
    }

    #[test]
    fn is_compatible_both_constraints_out_of_range() {
        let pkg = make_package("test", "1.0", vec![
            "remarkable-os>=3.10.0.0",
            "remarkable-os<4.0.0.0",
        ]);
        assert!(!pkg.is_compatible_with_os("3.9.0.0"));
        assert!(!pkg.is_compatible_with_os("4.0.0.0"));
    }

    #[test]
    fn parse_apkindex_single_package() {
        let input = "P:test-pkg\nV:1.0.0\nD:dep1 dep2\n";
        let reader = BufReader::new(input.as_bytes());
        let packages = parse_apkindex(reader).unwrap();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test-pkg");
        assert_eq!(packages[0].version, "1.0.0");
        assert_eq!(packages[0].depends, vec!["dep1", "dep2"]);
    }

    #[test]
    fn parse_apkindex_multiple_packages() {
        let input = "P:pkg1\nV:1.0\n\nP:pkg2\nV:2.0\n";
        let reader = BufReader::new(input.as_bytes());
        let packages = parse_apkindex(reader).unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "pkg1");
        assert_eq!(packages[1].name, "pkg2");
    }

    #[test]
    fn parse_apkindex_skips_malformed_lines() {
        let input = "P:test-pkg\nmalformed line\nV:1.0.0\nX\n";
        let reader = BufReader::new(input.as_bytes());
        let packages = parse_apkindex(reader).unwrap();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test-pkg");
        assert_eq!(packages[0].version, "1.0.0");
    }

    #[test]
    fn parse_apkindex_empty_input() {
        let input = "";
        let reader = BufReader::new(input.as_bytes());
        let packages = parse_apkindex(reader).unwrap();

        assert!(packages.is_empty());
    }

    #[test]
    fn parse_apkindex_no_trailing_newline() {
        let input = "P:test-pkg\nV:1.0.0";
        let reader = BufReader::new(input.as_bytes());
        let packages = parse_apkindex(reader).unwrap();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test-pkg");
    }

    #[test]
    fn parse_apkindex_ignores_unknown_fields() {
        let input = "P:test-pkg\nV:1.0.0\nA:x86_64\nS:12345\nI:67890\n";
        let reader = BufReader::new(input.as_bytes());
        let packages = parse_apkindex(reader).unwrap();

        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test-pkg");
        assert_eq!(packages[0].version, "1.0.0");
        assert!(packages[0].depends.is_empty());
    }
}
