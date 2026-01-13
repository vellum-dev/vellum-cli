use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process;

use crate::apk::{
    check_os_compatibility, generate_remarkable_os_package, fetch_remote_index,
    parse_index_tar_gz, Apk, Package,
};
use crate::device::get_apk_arch;
use crate::repo::update_index;
use crate::state::State;

const VELLUM_ROOT: &str = "/home/root/.vellum";

pub fn handle_upgrade(
    state: &State,
    apk: &Apk,
    args: &[String],
    os_mismatch: bool,
    os_prev: &str,
    os_cur: &str,
) {
    let mut upgrade_yes = false;
    let mut remaining_args = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-y" | "--yes" => upgrade_yes = true,
            _ => remaining_args.push(arg.clone()),
        }
    }

    if os_mismatch {
        println!("OS upgraded ({} -> {}). Checking package compatibility...", os_prev, os_cur);
        println!();

        let incompatible = check_os_compatibility_internal(apk, os_cur);
        if incompatible.is_none() {
            eprintln!("Could not fetch package index to verify compatibility.");
            eprintln!("Check your network connection and try again.");
            process::exit(1);
        }

        let incompatible = incompatible.unwrap();
        if !incompatible.is_empty() {
            println!("These packages have no version compatible with OS {}:", os_cur);
            for pkg in &incompatible {
                println!("  - {}", pkg);
            }
            println!();
            println!("Either wait for them to be updated, or remove them with 'vellum del <package>'.");
            println!("Then run 'vellum upgrade' again.");
            process::exit(1);
        }

        println!("All packages have compatible versions. Syncing OS version...");

        let arch = get_apk_arch();
        let repo_dir = format!("{}/local-repo/{}", VELLUM_ROOT, arch);
        let key_path = format!("{}/etc/apk/keys/local.rsa", VELLUM_ROOT);

        let _ = fs::create_dir_all(&repo_dir);
        remove_glob(&format!("{}/remarkable-os-*.apk", repo_dir));
        let _ = generate_remarkable_os_package(os_cur, &repo_dir);
        let _ = update_index(&repo_dir, Some(&key_path));
        let _ = state.set_os_version(os_cur);
        let _ = apk.run_silent(&["add", "remarkable-os"]);

        println!("Upgrading packages...");
    }

    let mut simulate_args = vec!["upgrade", "--simulate"];
    simulate_args.extend(remaining_args.iter().map(|s| s.as_str()));

    let output = match apk.output(&simulate_args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to check for upgrades: {}", e);
            process::exit(1);
        }
    };

    let mut packages = Vec::new();
    for line in output.lines() {
        if line.contains("Upgrading") {
            if let Some(rest) = line.split("Upgrading ").nth(1) {
                if let Some(pkg_name) = rest.split(" (").next() {
                    let pkg_name = pkg_name.trim();
                    if !pkg_name.is_empty() {
                        packages.push(pkg_name.to_string());
                    }
                }
            }
        }
    }

    if packages.is_empty() {
        println!("No packages to upgrade.");
        return;
    }

    if !upgrade_yes {
        println!("The following {} package(s) will be upgraded:", packages.len());
        for pkg in &packages {
            println!("  - {}", pkg);
        }
        print!("\nProceed with upgrade? [y/N] ");
        let _ = io::stdout().flush();

        let stdin = io::stdin();
        let mut line = String::new();
        let _ = stdin.lock().read_line(&mut line);
        let confirm = line.trim().to_lowercase();

        if confirm != "y" && confirm != "yes" {
            println!("Upgrade aborted.");
            process::exit(1);
        }
    }

    let mut upgrade_args = vec!["upgrade"];
    upgrade_args.extend(remaining_args.iter().map(|s| s.as_str()));

    if let Err(e) = apk.exec(&upgrade_args) {
        eprintln!("exec error: {}", e);
        process::exit(1);
    }
}

fn check_os_compatibility_internal(apk: &Apk, target_os: &str) -> Option<Vec<String>> {
    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(e) => {
            eprintln!("[debug] Failed to list installed packages: {}", e);
            return None;
        }
    };

    let virtual_pkgs: Vec<&str> = vec!["remarkable-os", "rm1", "rm2", "rmpp", "rmppm"];

    let filtered: Vec<String> = installed
        .into_iter()
        .filter(|p| !virtual_pkgs.contains(&p.as_str()))
        .collect();

    if filtered.is_empty() {
        return Some(Vec::new());
    }

    let index = match get_index() {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("[debug] Failed to get package index: {}", e);
            return None;
        }
    };

    let mut installed_with_os_dep = Vec::new();
    for pkg in &filtered {
        if let Ok(deps) = apk.get_dependencies(pkg) {
            if deps.iter().any(|d| d.contains("remarkable-os")) {
                installed_with_os_dep.push(pkg.clone());
            }
        }
    }

    if installed_with_os_dep.is_empty() {
        return Some(Vec::new());
    }

    let result = check_os_compatibility(target_os, &installed_with_os_dep, &index);
    Some(result.incompatible)
}

fn get_index() -> anyhow::Result<Vec<Package>> {
    let cache_dir = format!("{}/etc/apk/cache", VELLUM_ROOT);

    eprintln!("[debug] Looking for cached index in: {}", cache_dir);

    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("APKINDEX.") && name.ends_with(".tar.gz") {
                    eprintln!("[debug] Found cached index: {}", path.display());
                    return parse_index_tar_gz(path.to_str().unwrap());
                }
            }
        }
    }

    eprintln!("[debug] No cached index found, fetching from remote");

    let repo_url = match get_repo_url() {
        Some(url) => {
            eprintln!("[debug] Repository URL: {}", url);
            url
        }
        None => {
            let repos_file = format!("{}/etc/apk/repositories", VELLUM_ROOT);
            if let Ok(content) = fs::read_to_string(&repos_file) {
                eprintln!("[debug] repositories file contents:\n{}", content);
            } else {
                eprintln!("[debug] Could not read repositories file: {}", repos_file);
            }
            return Err(anyhow::anyhow!("no cached index and could not determine repository URL"));
        }
    };

    let arch = get_apk_arch();
    eprintln!("[debug] Fetching index for arch: {}", arch);

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

fn remove_glob(pattern: &str) {
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

fn matches_glob(name: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("*.apk") {
        name.starts_with(prefix) && name.ends_with(".apk")
    } else {
        name == pattern
    }
}
