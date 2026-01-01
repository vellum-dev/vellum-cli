use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{self, Command};

use crate::constants::VELLUM_ROOT;

pub fn handle_reenable() {
    let hooks_dir = format!("{VELLUM_ROOT}/hooks/post-os-upgrade");

    let entries = match fs::read_dir(&hooks_dir) {
        Ok(e) => e,
        Err(_) => {
            println!("No packages require re-enabling after OS upgrades.");
            process::exit(0);
        }
    };

    let entries: Vec<_> = entries.flatten().collect();
    if entries.is_empty() {
        println!("No packages require re-enabling after OS upgrades.");
        process::exit(0);
    }

    println!("Re-enabling packages after OS upgrade...");

    let mount_rw = format!("{VELLUM_ROOT}/bin/mount-rw");
    let mount_restore = format!("{VELLUM_ROOT}/bin/mount-restore");

    if run_command(&mount_rw).is_err() {
        eprintln!("warning: failed to remount filesystem read-write");
    }

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.permissions().mode() & 0o111 == 0 {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        println!("  {name}");

        if let Some(path_str) = path.to_str() {
            if run_command(path_str).is_err() {
                println!("    warning: {name} reenable script failed");
            }
        }
    }

    if run_command(&mount_restore).is_err() {
        eprintln!("warning: failed to restore filesystem mounts");
    }
    println!("Done.");
}

fn run_command(path: &str) -> anyhow::Result<()> {
    let status = Command::new(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("command failed"))
    }
}
