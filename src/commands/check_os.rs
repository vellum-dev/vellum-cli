use std::collections::HashMap;
use std::fs;
use std::process;

use crate::apk::{fetch_remote_index, parse_index_tar_gz, Apk, Package};
use crate::device::get_apk_arch;

const VELLUM_ROOT: &str = "/home/root/.vellum";

pub fn handle_check_os(apk: &Apk, target_os: &str) {
    println!("Checking package compatibility with OS {}...\n", target_os);

    let installed = match apk.list_installed() {
        Ok(pkgs) => pkgs,
        Err(_) => {
            eprintln!("Could not list installed packages.");
            process::exit(1);
        }
    };

    let virtual_pkgs: Vec<&str> = vec!["remarkable-os", "rm1", "rm2", "rmpp", "rmppm"];

    let user_pkgs: Vec<String> = installed
        .into_iter()
        .filter(|p| !virtual_pkgs.contains(&p.as_str()))
        .collect();

    if user_pkgs.is_empty() {
        println!("No user packages installed.");
        return;
    }

    let index = match get_index() {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Could not get package index: {}", e);
            process::exit(1);
        }
    };

    let mut pkg_versions: HashMap<&str, Vec<&Package>> = HashMap::new();
    for pkg in &index {
        pkg_versions.entry(&pkg.name).or_default().push(pkg);
    }

    let mut compatible = Vec::new();
    let mut incompatible = Vec::new();
    let mut no_constraint = Vec::new();

    for pkg in &user_pkgs {
        let versions = match pkg_versions.get(pkg.as_str()) {
            Some(v) => v,
            None => continue,
        };

        let has_os_constraint = versions.iter().any(|v| {
            let (min, max) = v.get_os_constraints();
            min.is_some() || max.is_some()
        });

        let has_compatible_version = versions.iter().any(|v| v.is_compatible_with_os(target_os));

        if !has_os_constraint {
            no_constraint.push(pkg.clone());
        } else if has_compatible_version {
            compatible.push(pkg.clone());
        } else {
            incompatible.push(pkg.clone());
        }
    }

    if !compatible.is_empty() {
        println!("Compatible packages:");
        for pkg in &compatible {
            println!("  + {}", pkg);
        }
        println!();
    }

    if !no_constraint.is_empty() {
        println!("Packages without OS constraints (assumed compatible):");
        for pkg in &no_constraint {
            println!("  - {}", pkg);
        }
        println!();
    }

    if !incompatible.is_empty() {
        println!("Incompatible packages (no version available for this OS):");
        for pkg in &incompatible {
            println!("  x {}", pkg);
        }
        println!();
        process::exit(1);
    }

    println!("All packages are compatible.");
}

fn get_index() -> anyhow::Result<Vec<Package>> {
    let cache_dir = format!("{}/etc/apk/cache", VELLUM_ROOT);

    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("APKINDEX.") && name.ends_with(".tar.gz") {
                    return parse_index_tar_gz(path.to_str().unwrap());
                }
            }
        }
    }

    let repo_url = get_repo_url().ok_or_else(|| {
        anyhow::anyhow!("no cached index and could not determine repository URL")
    })?;

    let arch = get_apk_arch();
    fetch_remote_index(&repo_url, &arch)
}

fn get_repo_url() -> Option<String> {
    let repos_file = format!("{}/etc/apk/repositories", VELLUM_ROOT);
    let content = fs::read_to_string(repos_file).ok()?;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.contains("local-repo") {
            return Some(line.to_string());
        }
    }
    None
}
