use std::collections::HashMap;

use super::index::Package;

#[derive(Debug, Default)]
pub struct CompatResult {
    pub compatible: Vec<String>,
    pub incompatible: Vec<String>,
    pub fetch_failed: bool,
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
