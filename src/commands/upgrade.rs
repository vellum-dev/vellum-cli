use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

use crate::apk::{
    check_os_compatibility, generate_remarkable_os_package, fetch_remote_index,
    parse_index_tar_gz, version_lt, Apk, Package,
};
use crate::constants::{VELLUM_ROOT, VIRTUAL_PKGS};
use crate::device::get_apk_arch;
use crate::repo::update_index;
use crate::state::State;
use crate::util::remove_glob;

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

    let is_downgrade = os_mismatch && version_lt(os_cur, os_prev);

    if os_mismatch {
        let action = if is_downgrade { "downgraded" } else { "upgraded" };
        println!("OS {action} ({os_prev} -> {os_cur}). Checking package compatibility...");
        println!();

        let incompatible = check_os_compatibility_internal(apk, os_cur);
        if incompatible.is_none() {
            eprintln!("Could not fetch package index to verify compatibility.");
            eprintln!("Check your network connection and try again.");
            process::exit(1);
        }

        let incompatible = incompatible.unwrap();
        if !incompatible.is_empty() {
            println!("These packages have no version compatible with OS {os_cur}:");
            for pkg in &incompatible {
                println!("  - {pkg}");
            }
            println!();
            println!("Either wait for them to be updated, or remove them with 'vellum del <package>'.");
            println!("Then run 'vellum upgrade' again.");
            process::exit(1);
        }

        println!("All packages have compatible versions. Preparing upgrade...");

        let arch = get_apk_arch();
        let repo_dir = format!("{VELLUM_ROOT}/local-repo/{arch}");
        let key_path = format!("{VELLUM_ROOT}/etc/apk/keys/local.rsa");

        if let Err(e) = fs::create_dir_all(&repo_dir) {
            eprintln!("warning: failed to create repo directory: {e}");
        }
        remove_glob(&format!("{repo_dir}/remarkable-os-*.apk"));
        if let Err(e) = generate_remarkable_os_package(os_cur, &repo_dir, &key_path) {
            eprintln!("warning: failed to generate remarkable-os package: {e}");
        }
        if let Err(e) = update_index(&repo_dir, Some(&key_path)) {
            eprintln!("warning: failed to update local repo index: {e}");
        }

        clean_world_file_pins(apk);

        if is_downgrade {
            let pkg_version = format!("remarkable-os={os_cur}-r0");
            if let Err(e) = apk.run(&["add", &pkg_version]) {
                eprintln!("warning: failed to downgrade remarkable-os package: {e}");
            }
        }
    }

    let mut simulate_args = vec!["upgrade", "--simulate"];
    if is_downgrade {
        simulate_args.push("--available");
    }
    simulate_args.extend(remaining_args.iter().map(|s| s.as_str()));

    let output = match apk.output(&simulate_args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to check for upgrades: {e}");
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
        if os_mismatch {
            match apk.get_package_version("remarkable-os") {
                Ok(Some(installed_ver)) if installed_ver == os_cur => {
                    if let Err(e) = state.set_os_version(os_cur) {
                        eprintln!("warning: failed to save OS version: {e}");
                    }
                    println!("OS version synced to {os_cur}");
                }
                _ => {}
            }
        }
        println!("No packages to upgrade.");
        return;
    }

    if !upgrade_yes {
        println!("The following {} package(s) will be upgraded:", packages.len());
        for pkg in &packages {
            println!("  - {pkg}");
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
    if is_downgrade {
        upgrade_args.push("--available");
    }
    upgrade_args.extend(remaining_args.iter().map(|s| s.as_str()));

    if os_mismatch {
        if let Err(e) = apk.run(&upgrade_args) {
            eprintln!("upgrade error: {e}");
            process::exit(1);
        }

        match apk.get_package_version("remarkable-os") {
            Ok(Some(installed_ver)) if installed_ver == os_cur => {
                if let Err(e) = state.set_os_version(os_cur) {
                    eprintln!("warning: failed to save OS version: {e}");
                }
                println!("OS version synced to {os_cur}");
            }
            Ok(Some(installed_ver)) => {
                eprintln!("error: remarkable-os package is at {installed_ver}, expected {os_cur}");
                eprintln!("OS version sync failed. Run 'vellum upgrade' to retry.");
                process::exit(1);
            }
            Ok(None) => {
                eprintln!("error: remarkable-os package not found after upgrade");
                process::exit(1);
            }
            Err(e) => {
                eprintln!("warning: could not verify remarkable-os version: {e}");
            }
        }
    } else {
        if let Err(e) = apk.exec(&upgrade_args) {
            eprintln!("exec error: {e}");
            process::exit(1);
        }
    }
}

fn check_os_compatibility_internal(apk: &Apk, target_os: &str) -> Option<Vec<String>> {
    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(_) => return None,
    };

    let filtered: Vec<String> = installed
        .into_iter()
        .filter(|p| !VIRTUAL_PKGS.contains(&p.as_str()))
        .collect();

    if filtered.is_empty() {
        return Some(Vec::new());
    }

    let index = match get_index() {
        Ok(idx) => idx,
        Err(_) => return None,
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

fn clean_world_file_pins(apk: &Apk) {
    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(_) => return,
    };

    let mut packages_to_unpin: Vec<String> = installed
        .into_iter()
        .filter(|p| !VIRTUAL_PKGS.contains(&p.as_str()))
        .filter(|p| {
            if let Ok(deps) = apk.get_dependencies(p) {
                deps.iter().any(|d| d.contains("remarkable-os"))
            } else {
                false
            }
        })
        .collect();

    packages_to_unpin.push("remarkable-os".to_string());

    let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
    let content = match fs::read_to_string(&world_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let new_content: String = content
        .lines()
        .map(|line| {
            for pkg in &packages_to_unpin {
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

