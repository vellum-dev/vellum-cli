use std::fs;
use std::process::{self, Command};

use crate::apk::{
    fetch_remote_index, find_best_compatible_version, parse_apk_file, parse_index_tar_gz,
    resolve_compatible_deps, Apk, Package,
};
use crate::constants::VELLUM_ROOT;
use crate::device::get_apk_arch;
use crate::util::{restore_world_file_entries, strip_world_file_pins, world_file_members};

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
    let mut resolved_pkgs: Vec<&Package> = Vec::new();
    let mut local_apk_packages: Vec<Package> = Vec::new();
    let mut has_incompatible = false;

    for arg in args {
        if arg.contains('=') || arg.contains('<') || arg.contains('>') || arg.starts_with('-') {
            resolved_args.push(arg.clone());
            continue;
        }

        if arg.ends_with(".apk") {
            resolved_args.push(arg.clone());
            if let Ok(pkg) = parse_apk_file(arg) {
                local_apk_packages.push(pkg);
            }
            continue;
        }

        match find_best_compatible_version(arg, &os_version, &index) {
            Some(pkg) => {
                resolved_args.push(format!("{}={}", pkg.name, pkg.version));
                resolved_packages.push(pkg.name.clone());
                resolved_pkgs.push(pkg);
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

    let transitive_pins = resolve_compatible_deps(&resolved_pkgs, &os_version, &index);
    let mut transitive_names: Vec<String> = Vec::new();
    for pin in &transitive_pins {
        let name = pin.split('=').next().unwrap_or(pin);
        if !resolved_packages.iter().any(|r| r == name) {
            resolved_args.push(pin.clone());
            transitive_names.push(name.to_string());
        }
    }

    let fetch_args: Vec<&str> = resolved_args
        .iter()
        .filter(|a| !a.starts_with('-') && !a.ends_with(".apk"))
        .map(|s| s.as_str())
        .collect();
    if !fetch_args.is_empty() {
        let _ = apk.fetch(&fetch_args);
    }

    let cached_packages = find_cached_apk_packages(apk, &resolved_args);
    let mut packages_to_replace = find_packages_to_replace(apk, &cached_packages);
    packages_to_replace.extend(find_packages_to_replace(apk, &local_apk_packages));
    if !packages_to_replace.is_empty() {
        println!("The following packages will be replaced:");
        for pkg in &packages_to_replace {
            println!("  - {pkg}");
        }
        println!();

        let del_args: Vec<&str> = packages_to_replace.iter().map(|s| s.as_str()).collect();
        let mut del_cmd = vec!["del"];
        del_cmd.extend(del_args);
        if let Err(e) = apk.run(&del_cmd) {
            eprintln!("Failed to remove replaced packages: {e}");
            process::exit(1);
        }
    }

    let world_path = format!("{VELLUM_ROOT}/etc/apk/world");
    let members_before_add =
        world_file_members(&fs::read_to_string(&world_path).unwrap_or_default());

    run_apk_add(apk, &resolved_args.iter().map(|s| s.as_str()).collect::<Vec<_>>());

    if !resolved_packages.is_empty() {
        strip_world_file_pins(&resolved_packages);
    }
    if !transitive_names.is_empty() {
        restore_world_file_entries(&transitive_names, &members_before_add);
    }
}

fn run_add_directly(apk: &Apk, args: &[String]) {
    run_apk_add(apk, &args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
}

fn run_apk_add(apk: &Apk, args: &[&str]) {
    let had_hooks_before = has_post_os_upgrade_hooks();

    let mut cmd_args = vec!["add", "--cache-predownload"];
    cmd_args.extend(args);

    let result = apk.run(&cmd_args);
    let _ = apk.cache_purge();

    if result.is_err() {
        process::exit(1);
    }

    if !had_hooks_before && has_post_os_upgrade_hooks() {
        touch_reenable_marker();
    }
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

fn find_packages_to_replace(apk: &Apk, cached_packages: &[Package]) -> Vec<String> {
    let installed = match apk.list_installed() {
        Ok(list) => list,
        Err(_) => return Vec::new(),
    };

    let mut to_replace = Vec::new();

    for pkg in cached_packages {
        for replaced in &pkg.replaces {
            if installed.contains(replaced) && !to_replace.contains(replaced) {
                to_replace.push(replaced.clone());
            }
        }
    }

    to_replace
}

fn find_cached_apk_packages(apk: &Apk, package_specs: &[String]) -> Vec<Package> {
    let cache_dir = apk.cache_dir();
    let entries = match fs::read_dir(&cache_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let apk_files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if name.ends_with(".apk") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    let mut packages = Vec::new();

    for spec in package_specs {
        let pkg_name = spec.split('=').next().unwrap_or(spec);

        for apk_path in &apk_files {
            let file_name = match apk_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            if file_name.starts_with(&format!("{pkg_name}-")) {
                if let Some(path_str) = apk_path.to_str() {
                    if let Ok(pkg) = parse_apk_file(path_str) {
                        if pkg.name == pkg_name {
                            packages.push(pkg);
                            break;
                        }
                    }
                }
            }
        }
    }

    packages
}

fn has_post_os_upgrade_hooks() -> bool {
    let hooks_dir = format!("{VELLUM_ROOT}/hooks/post-os-upgrade");
    fs::read_dir(&hooks_dir)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

fn touch_reenable_marker() {
    let mount_rw = format!("{VELLUM_ROOT}/bin/mount-rw");
    let mount_restore = format!("{VELLUM_ROOT}/bin/mount-restore");

    if run_command(&mount_rw).is_err() {
        return;
    }

    let marker_dir = "/etc/vellum";
    let _ = fs::create_dir_all(marker_dir);
    let _ = fs::write(format!("{marker_dir}/reenabled"), "");

    let _ = run_command(&mount_restore);
}

fn run_command(path: &str) -> anyhow::Result<()> {
    let status = Command::new(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("command failed"))
    }
}
