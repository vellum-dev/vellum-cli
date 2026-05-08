use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

use crate::apk::{
    check_os_compatibility, find_best_compatible_version, generate_remarkable_os_package,
    fetch_remote_index, parse_index_tar_gz, resolve_compatible_deps, version_lt, Apk, Package,
};
use crate::constants::{VELLUM_ROOT, VIRTUAL_PKGS};
use crate::device::get_apk_arch;
use crate::repo::update_index;
use crate::state::State;
use crate::util::{remove_glob, remove_world_file_entries, strip_world_file_pins};

pub fn handle_upgrade(
    state: &State,
    apk: &Apk,
    args: &[String],
    os_mismatch: bool,
    os_prev: &str,
    os_cur: &str,
) {
    let mut upgrade_yes = false;
    let mut simulate = false;
    let mut remaining_args = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-y" | "--yes" => upgrade_yes = true,
            "--simulate" => simulate = true,
            _ => remaining_args.push(arg.clone()),
        }
    }

    let is_downgrade = os_mismatch && version_lt(os_cur, os_prev);

    let index = match get_index() {
        Ok(idx) => Some(idx),
        Err(_) if os_mismatch => {
            eprintln!("Could not fetch package index to verify compatibility.");
            eprintln!("Check your network connection and try again.");
            process::exit(1);
        }
        Err(_) => None,
    };

    if os_mismatch {
        let action = if is_downgrade { "downgraded" } else { "upgraded" };
        println!("OS {action} ({os_prev} -> {os_cur}). Checking package compatibility...");
        println!();

        let incompatible = check_os_compatibility_internal(apk, os_cur, index.as_deref().unwrap());

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

        let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
        let original_world = fs::read_to_string(&world_path).unwrap_or_default();

        clean_world_file_pins(apk);
        pin_world_file_entry(&world_path, "remarkable-os", &format!("{os_cur}-r0"));

        let installed = apk.list_installed().unwrap_or_default();
        let os_pins = pin_upgrade_packages(&installed, os_cur, index.as_deref());
        for pin in os_pins.direct.iter().chain(&os_pins.transitive) {
            let name = pin.split('=').next().unwrap_or(pin);
            let version = pin.split('=').nth(1).unwrap_or("");
            if !version.is_empty() {
                pin_world_file_entry(&world_path, name, version);
            }
        }

        match find_replacement_packages(apk, os_cur, index.as_deref()) {
            ReplacementResult::Conflict(conflicts) => {
                let _ = fs::write(&world_path, &original_world);
                eprintln!("Multiple packages want to replace the same installed package:");
                for (old_pkg, candidates) in &conflicts {
                    eprintln!("  {old_pkg} can be replaced by: {}", candidates.join(", "));
                }
                eprintln!();
                eprintln!("Please choose and install one manually:");
                for (_, candidates) in &conflicts {
                    for candidate in candidates {
                        eprintln!("  vellum add {candidate}");
                    }
                }
                process::exit(1);
            }
            ReplacementResult::Ok(replacements) if !replacements.is_empty() => {
                println!("Package replacements available:");
                for (new_pkg, old_pkg) in &replacements {
                    println!("  {old_pkg} -> {new_pkg}");
                }
                println!();

                for (new_pkg, _) in &replacements {
                    remaining_args.push(new_pkg.clone());
                }
            }
            ReplacementResult::Ok(_) => {}
        }

        let mut sim_args = vec!["upgrade", "--simulate"];
        if is_downgrade {
            sim_args.push("--available");
        }
        sim_args.extend(remaining_args.iter().map(|s| s.as_str()));

        let result = apk.output(&sim_args);

        if simulate {
            let _ = fs::write(&world_path, &original_world);

            match result {
                Ok(output) => {
                    print!("{output}");
                }
                Err(e) => {
                    eprintln!("Failed to simulate upgrade: {e}");
                    process::exit(1);
                }
            }
            return;
        }

        let output = match result {
            Ok(o) => o,
            Err(e) => {
                let _ = fs::write(&world_path, &original_world);
                eprintln!("Failed to check for upgrades: {e}");
                process::exit(1);
            }
        };

        let mut packages = Vec::new();
        for line in output.lines() {
            for action_str in ["Upgrading ", "Installing ", "Downgrading "] {
                if line.contains(action_str) {
                    if let Some(rest) = line.split(action_str).nth(1) {
                        if let Some(pkg_name) = rest.split(" (").next() {
                            let pkg_name = pkg_name.trim();
                            if !pkg_name.is_empty() && !packages.contains(&pkg_name.to_string()) {
                                packages.push(pkg_name.to_string());
                            }
                        }
                    }
                }
            }
        }

        let packages_to_replace = find_packages_to_replace(apk, &packages, os_cur, index.as_deref());

        if packages.is_empty() && packages_to_replace.is_empty() {
            let _ = fs::write(&world_path, &original_world);
            match apk.get_package_version("remarkable-os") {
                Ok(Some(installed_ver)) if installed_ver == os_cur => {
                    if let Err(e) = state.set_os_version(os_cur) {
                        eprintln!("warning: failed to save OS version: {e}");
                    }
                    println!("OS version synced to {os_cur}");
                }
                _ => {}
            }
            println!("No packages to upgrade.");
            return;
        }

        if !upgrade_yes {
            println!("The following {} package(s) will be upgraded:", packages.len());
            for pkg in &packages {
                println!("  - {pkg}");
            }
            if !packages_to_replace.is_empty() {
                println!("\nThe following {} package(s) will be removed (replaced):", packages_to_replace.len());
                for pkg in &packages_to_replace {
                    println!("  - {pkg}");
                }
            }
            print!("\nProceed with upgrade? [Y/n] ");
            let _ = io::stdout().flush();

            let stdin = io::stdin();
            let mut line = String::new();
            let _ = stdin.lock().read_line(&mut line);
            let confirm = line.trim().to_lowercase();

            if confirm == "n" || confirm == "no" {
                let _ = fs::write(&world_path, &original_world);
                println!("Upgrade aborted.");
                process::exit(1);
            }
        }

        if !packages_to_replace.is_empty() {
            println!("Removing replaced packages...");
            let del_args: Vec<&str> = packages_to_replace.iter().map(|s| s.as_str()).collect();
            let mut del_cmd = vec!["del"];
            del_cmd.extend(del_args);
            if let Err(e) = apk.run(&del_cmd) {
                eprintln!("warning: failed to remove replaced packages: {e}");
            }
        }

        let mut upgrade_args = vec!["upgrade"];
        if is_downgrade {
            upgrade_args.push("--available");
        }
        upgrade_args.extend(remaining_args.iter().map(|s| s.as_str()));

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

        let pinned_names: Vec<String> = os_pins.direct
            .iter()
            .filter_map(|s| s.split('=').next().map(|n| n.to_string()))
            .collect();
        if !pinned_names.is_empty() {
            strip_world_file_pins(&pinned_names);
        }
        let transitive_names: Vec<String> = os_pins.transitive
            .iter()
            .filter_map(|s| s.split('=').next().map(|n| n.to_string()))
            .collect();
        if !transitive_names.is_empty() {
            remove_world_file_entries(&transitive_names);
        }

        strip_world_file_pins(&["remarkable-os".to_string()]);

        return;
    }

    let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
    let original_world = fs::read_to_string(&world_path).unwrap_or_default();

    clean_world_file_pins(apk);
    pin_world_file_entry(&world_path, "remarkable-os", &format!("{os_cur}-r0"));

    let installed = apk.list_installed().unwrap_or_default();
    let os_pins = pin_upgrade_packages(&installed, os_cur, index.as_deref());
    for pin in os_pins.direct.iter().chain(&os_pins.transitive) {
        let name = pin.split('=').next().unwrap_or(pin);
        let version = pin.split('=').nth(1).unwrap_or("");
        if !version.is_empty() {
            pin_world_file_entry(&world_path, name, version);
        }
    }

    match find_replacement_packages(apk, os_cur, index.as_deref()) {
        ReplacementResult::Conflict(conflicts) => {
            let _ = fs::write(&world_path, &original_world);
            eprintln!("Multiple packages want to replace the same installed package:");
            for (old_pkg, candidates) in &conflicts {
                eprintln!("  {old_pkg} can be replaced by: {}", candidates.join(", "));
            }
            eprintln!();
            eprintln!("Please choose and install one manually:");
            for (_, candidates) in &conflicts {
                for candidate in candidates {
                    eprintln!("  vellum add {candidate}");
                }
            }
            process::exit(1);
        }
        ReplacementResult::Ok(replacements) if !replacements.is_empty() => {
            println!("Package replacements available:");
            for (new_pkg, old_pkg) in &replacements {
                println!("  {old_pkg} -> {new_pkg}");
            }
            println!();

            for (new_pkg, _) in &replacements {
                remaining_args.push(new_pkg.clone());
            }
        }
        ReplacementResult::Ok(_) => {}
    }

    let mut simulate_args = vec!["upgrade", "--simulate"];
    simulate_args.extend(remaining_args.iter().map(|s| s.as_str()));

    let output = match apk.output(&simulate_args) {
        Ok(o) => o,
        Err(e) => {
            let _ = fs::write(&world_path, &original_world);
            eprintln!("Failed to check for upgrades: {e}");
            process::exit(1);
        }
    };

    let mut packages = Vec::new();
    for line in output.lines() {
        for action in ["Upgrading ", "Installing "] {
            if line.contains(action) {
                if let Some(rest) = line.split(action).nth(1) {
                    if let Some(pkg_name) = rest.split(" (").next() {
                        let pkg_name = pkg_name.trim();
                        if !pkg_name.is_empty() && !packages.contains(&pkg_name.to_string()) {
                            packages.push(pkg_name.to_string());
                        }
                    }
                }
            }
        }
    }

    let packages_to_replace = find_packages_to_replace(apk, &packages, os_cur, index.as_deref());

    if packages.is_empty() && packages_to_replace.is_empty() {
        let _ = fs::write(&world_path, &original_world);
        println!("No packages to upgrade.");
        return;
    }

    if simulate {
        let _ = fs::write(&world_path, &original_world);
        print!("{output}");
        return;
    }

    if !upgrade_yes {
        println!("The following {} package(s) will be upgraded:", packages.len());
        for pkg in &packages {
            println!("  - {pkg}");
        }
        if !packages_to_replace.is_empty() {
            println!("\nThe following {} package(s) will be removed (replaced):", packages_to_replace.len());
            for pkg in &packages_to_replace {
                println!("  - {pkg}");
            }
        }
        print!("\nProceed with upgrade? [Y/n] ");
        let _ = io::stdout().flush();

        let stdin = io::stdin();
        let mut line = String::new();
        let _ = stdin.lock().read_line(&mut line);
        let confirm = line.trim().to_lowercase();

        if confirm == "n" || confirm == "no" {
            let _ = fs::write(&world_path, &original_world);
            println!("Upgrade aborted.");
            process::exit(1);
        }
    }

    if !packages_to_replace.is_empty() {
        println!("Removing replaced packages...");
        let del_args: Vec<&str> = packages_to_replace.iter().map(|s| s.as_str()).collect();
        let mut del_cmd = vec!["del"];
        del_cmd.extend(del_args);
        if let Err(e) = apk.run(&del_cmd) {
            eprintln!("warning: failed to remove replaced packages: {e}");
        }
    }

    let mut upgrade_args = vec!["upgrade"];
    upgrade_args.extend(remaining_args.iter().map(|s| s.as_str()));

    if let Err(e) = apk.run(&upgrade_args) {
        eprintln!("upgrade error: {e}");
        process::exit(1);
    }

    let pinned_names: Vec<String> = os_pins.direct
        .iter()
        .filter_map(|s| s.split('=').next().map(|n| n.to_string()))
        .collect();
    if !pinned_names.is_empty() {
        strip_world_file_pins(&pinned_names);
    }
    let transitive_names: Vec<String> = os_pins.transitive
        .iter()
        .filter_map(|s| s.split('=').next().map(|n| n.to_string()))
        .collect();
    if !transitive_names.is_empty() {
        remove_world_file_entries(&transitive_names);
    }

    strip_world_file_pins(&["remarkable-os".to_string()]);
}

struct PinResult {
    direct: Vec<String>,
    transitive: Vec<String>,
}

fn pin_upgrade_packages(packages: &[String], os_version: &str, index: Option<&[Package]>) -> PinResult {
    let index = match index {
        Some(idx) => idx,
        None => return PinResult { direct: Vec::new(), transitive: Vec::new() },
    };

    let mut direct = Vec::new();
    let mut resolved_pkgs = Vec::new();

    for pkg_name in packages {
        if pkg_name == "remarkable-os" || VIRTUAL_PKGS.contains(&pkg_name.as_str()) {
            continue;
        }
        let has_os_constrained = index
            .iter()
            .any(|p| p.name == *pkg_name && p.get_os_constraints() != (None, None));
        if !has_os_constrained {
            continue;
        }
        if let Some(best) = find_best_compatible_version(pkg_name, os_version, index) {
            direct.push(format!("{}={}", best.name, best.version));
            resolved_pkgs.push(best);
        }
    }

    let mut transitive = Vec::new();
    for pin in resolve_compatible_deps(&resolved_pkgs, os_version, index) {
        let name = pin.split('=').next().unwrap_or(&pin);
        if !direct.iter().any(|p| p.starts_with(&format!("{name}="))) {
            transitive.push(pin);
        }
    }

    PinResult { direct, transitive }
}

fn check_os_compatibility_internal(apk: &Apk, target_os: &str, index: &[Package]) -> Vec<String> {
    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(_) => return Vec::new(),
    };

    let filtered: Vec<String> = installed
        .into_iter()
        .filter(|p| !VIRTUAL_PKGS.contains(&p.as_str()))
        .collect();

    if filtered.is_empty() {
        return Vec::new();
    }

    let mut installed_with_os_dep = Vec::new();
    for pkg in &filtered {
        if let Ok(deps) = apk.get_dependencies(pkg) {
            if deps.iter().any(|d| d.contains("remarkable-os")) {
                installed_with_os_dep.push(pkg.clone());
            }
        }
    }

    if installed_with_os_dep.is_empty() {
        return Vec::new();
    }

    let result = check_os_compatibility(target_os, &installed_with_os_dep, index);
    result.incompatible
}

fn get_index() -> anyhow::Result<Vec<Package>> {
    let cache_dir = format!("{VELLUM_ROOT}/etc/apk/cache");
    let mut all_packages = Vec::new();

    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("APKINDEX.") && name.ends_with(".tar.gz") {
                    if let Some(path_str) = path.to_str() {
                        if let Ok(packages) = parse_index_tar_gz(path_str) {
                            all_packages.extend(packages);
                        }
                    }
                }
            }
        }
    }

    if !all_packages.is_empty() {
        return Ok(all_packages);
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

fn pin_world_file_entry(world_path: &str, pkg: &str, version: &str) {
    let content = fs::read_to_string(world_path).unwrap_or_default();
    let pinned = format!("{pkg}={version}");
    let mut found = false;

    let new_content: String = content
        .lines()
        .map(|line| {
            if line == pkg || line.starts_with(&format!("{pkg}=")) {
                found = true;
                pinned.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let final_content = if found {
        new_content + "\n"
    } else if new_content.is_empty() {
        format!("{pinned}\n")
    } else {
        format!("{new_content}\n{pinned}\n")
    };

    let _ = fs::write(world_path, final_content);
}

fn find_packages_to_replace(apk: &Apk, upgrading_packages: &[String], os_version: &str, index: Option<&[Package]>) -> Vec<String> {
    let index = match index {
        Some(idx) => idx,
        None => return Vec::new(),
    };

    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(_) => return Vec::new(),
    };

    let mut to_replace = Vec::new();

    for pkg_name in upgrading_packages {
        if let Some(pkg) = find_best_compatible_version(pkg_name, os_version, index) {
            for replaced in &pkg.replaces {
                if installed.contains(replaced) && !to_replace.contains(replaced) {
                    to_replace.push(replaced.clone());
                }
            }
        }
    }

    to_replace
}

enum ReplacementResult {
    Ok(Vec<(String, String)>),
    Conflict(Vec<(String, Vec<String>)>),
}

fn find_replacement_packages(apk: &Apk, os_version: &str, index: Option<&[Package]>) -> ReplacementResult {
    let index = match index {
        Some(idx) => idx,
        None => return ReplacementResult::Ok(Vec::new()),
    };

    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(_) => return ReplacementResult::Ok(Vec::new()),
    };

    let mut replacements_map: HashMap<String, Vec<String>> = HashMap::new();

    for pkg in index {
        if !pkg.is_compatible_with_os(os_version) {
            continue;
        }
        for replaced in &pkg.replaces {
            if installed.contains(replaced) {
                replacements_map
                    .entry(replaced.clone())
                    .or_default()
                    .push(pkg.name.clone());
            }
        }
    }

    let conflicts: Vec<_> = replacements_map
        .iter()
        .filter(|(_, replacers)| replacers.len() > 1)
        .map(|(old, replacers)| (old.clone(), replacers.clone()))
        .collect();

    if !conflicts.is_empty() {
        return ReplacementResult::Conflict(conflicts);
    }

    let result = replacements_map
        .into_iter()
        .map(|(old, mut replacers)| (replacers.remove(0), old))
        .collect();

    ReplacementResult::Ok(result)
}

