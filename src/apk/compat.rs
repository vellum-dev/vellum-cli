use std::collections::HashMap;

use super::index::Package;

#[derive(Debug, Default)]
pub struct CompatResult {
    pub compatible: Vec<String>,
    pub incompatible: Vec<String>,
}

pub fn check_os_compatibility(
    target_os: &str,
    installed_pkgs: &[String],
    index: &[Package],
) -> CompatResult {
    let mut result = CompatResult::default();

    let mut pkg_versions: HashMap<&str, Vec<&Package>> = HashMap::new();
    for pkg in index {
        pkg_versions.entry(&pkg.name).or_default().push(pkg);
    }

    for installed in installed_pkgs {
        let versions = match pkg_versions.get(installed.as_str()) {
            Some(v) => v,
            None => continue,
        };

        let has_os = versions.iter().any(|v| {
            let (min, max) = v.get_os_constraints();
            min.is_some() || max.is_some()
        });

        if !has_os {
            continue;
        }

        let has_compatible = versions.iter().any(|v| v.is_compatible_with_os(target_os));

        if has_compatible {
            result.compatible.push(installed.clone());
        } else {
            result.incompatible.push(installed.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_package(name: &str, version: &str, depends: Vec<&str>) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
            depends: depends.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn all_packages_compatible() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["remarkable-os>=3.0.0.0"]),
            make_package("pkg2", "1.0", vec!["remarkable-os>=3.0.0.0"]),
        ];
        let installed = vec!["pkg1".to_string(), "pkg2".to_string()];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert_eq!(result.compatible, vec!["pkg1", "pkg2"]);
        assert!(result.incompatible.is_empty());
    }

    #[test]
    fn some_packages_incompatible() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["remarkable-os>=3.0.0.0"]),
            make_package("pkg2", "1.0", vec!["remarkable-os>=4.0.0.0"]),
        ];
        let installed = vec!["pkg1".to_string(), "pkg2".to_string()];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert_eq!(result.compatible, vec!["pkg1"]);
        assert_eq!(result.incompatible, vec!["pkg2"]);
    }

    #[test]
    fn package_not_in_index_skipped() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["remarkable-os>=3.0.0.0"]),
        ];
        let installed = vec!["pkg1".to_string(), "unknown-pkg".to_string()];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert_eq!(result.compatible, vec!["pkg1"]);
        assert!(result.incompatible.is_empty());
    }

    #[test]
    fn package_without_os_constraints_skipped() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["other-dep"]),
            make_package("pkg2", "1.0", vec!["remarkable-os>=3.0.0.0"]),
        ];
        let installed = vec!["pkg1".to_string(), "pkg2".to_string()];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert_eq!(result.compatible, vec!["pkg2"]);
        assert!(result.incompatible.is_empty());
    }

    #[test]
    fn multiple_versions_any_compatible_means_compatible() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["remarkable-os>=3.0.0.0", "remarkable-os<3.5.0.0"]),
            make_package("pkg1", "2.0", vec!["remarkable-os>=3.5.0.0"]),
        ];
        let installed = vec!["pkg1".to_string()];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert_eq!(result.compatible, vec!["pkg1"]);
        assert!(result.incompatible.is_empty());
    }

    #[test]
    fn multiple_versions_none_compatible_means_incompatible() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["remarkable-os>=3.0.0.0", "remarkable-os<3.5.0.0"]),
            make_package("pkg1", "2.0", vec!["remarkable-os>=3.5.0.0", "remarkable-os<4.0.0.0"]),
        ];
        let installed = vec!["pkg1".to_string()];

        let result = check_os_compatibility("4.0.0.0", &installed, &index);

        assert!(result.compatible.is_empty());
        assert_eq!(result.incompatible, vec!["pkg1"]);
    }

    #[test]
    fn empty_installed_list() {
        let index = vec![
            make_package("pkg1", "1.0", vec!["remarkable-os>=3.0.0.0"]),
        ];
        let installed: Vec<String> = vec![];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert!(result.compatible.is_empty());
        assert!(result.incompatible.is_empty());
    }

    #[test]
    fn empty_index() {
        let index: Vec<Package> = vec![];
        let installed = vec!["pkg1".to_string()];

        let result = check_os_compatibility("3.10.0.0", &installed, &index);

        assert!(result.compatible.is_empty());
        assert!(result.incompatible.is_empty());
    }
}
