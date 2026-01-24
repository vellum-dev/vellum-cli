use std::fs;
use std::process;

use crate::apk::{fetch_remote_index, find_best_compatible_version, parse_index_tar_gz, Apk, Package};
use crate::constants::VELLUM_ROOT;
use crate::device::get_apk_arch;

pub fn handle_add(apk: &Apk, args: &[String]) {
    let os_version = match apk.get_package_version("remarkable-os") {
        Ok(Some(v)) => v,
        Ok(None) | Err(_) => {
            return run_add_directly(apk, args);
        }
    };

    let index = match get_index() {
        Ok(idx) => idx,
        Err(_) => {
            return run_add_directly(apk, args);
        }
    };

    let mut resolved_args: Vec<String> = Vec::new();
    let mut resolved_packages: Vec<String> = Vec::new();
    let mut has_incompatible = false;

    for arg in args {
        if arg.contains('=') || arg.contains('<') || arg.contains('>') || arg.starts_with('-') {
            resolved_args.push(arg.clone());
            continue;
        }

        match find_best_compatible_version(arg, &os_version, &index) {
            Some(pkg) => {
                resolved_args.push(format!("{}={}", pkg.name, pkg.version));
                resolved_packages.push(pkg.name.clone());
            }
            None => {
                let has_any_version = index.iter().any(|p| p.name == *arg);
                if has_any_version {
                    eprintln!("Error: No version of '{arg}' is compatible with OS {os_version}");
                    has_incompatible = true;
                } else {
                    resolved_args.push(arg.clone());
                }
            }
        }
    }

    if has_incompatible {
        process::exit(1);
    }

    let mut cmd_args = vec!["add", "--cache-predownload"];
    cmd_args.extend(resolved_args.iter().map(|s| s.as_str()));

    let result = apk.run(&cmd_args);
    let _ = apk.cache_purge();

    if result.is_err() {
        process::exit(1);
    }

    if !resolved_packages.is_empty() {
        clean_world_file_pins(&resolved_packages);
    }
}

fn run_add_directly(apk: &Apk, args: &[String]) {
    let mut cmd_args = vec!["add", "--cache-predownload"];
    cmd_args.extend(args.iter().map(|s| s.as_str()));

    let result = apk.run(&cmd_args);
    let _ = apk.cache_purge();

    if result.is_err() {
        process::exit(1);
    }
}

fn get_index() -> anyhow::Result<Vec<Package>> {
    let cache_dir = format!("{VELLUM_ROOT}/etc/apk/cache");

    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("APKINDEX.") && name.ends_with(".tar.gz") {
                    if let Some(path_str) = path.to_str() {
                        return parse_index_tar_gz(path_str);
                    }
                }
            }
        }
    }

    let repo_url = match get_repo_url() {
        Some(url) => url,
        None => return Err(anyhow::anyhow!("no cached index and could not determine repository URL")),
    };

    let arch = get_apk_arch();
    fetch_remote_index(&repo_url, &arch)
}

fn get_repo_url() -> Option<String> {
    let repos_file = format!("{VELLUM_ROOT}/etc/apk/repositories");
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

fn clean_world_file_pins(packages: &[String]) {
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
