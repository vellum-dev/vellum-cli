use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{self, Command};

const VELLUM_ROOT: &str = "/home/root/.vellum";

pub fn handle_reenable() {
    let hooks_dir = format!("{}/hooks/post-os-upgrade", VELLUM_ROOT);

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

    let mount_rw = format!("{}/bin/mount-rw", VELLUM_ROOT);
    let mount_restore = format!("{}/bin/mount-restore", VELLUM_ROOT);

    let _ = run_command(&mount_rw);

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
        println!("  {}", name);

        if run_command(path.to_str().unwrap()).is_err() {
            println!("    warning: {} reenable script failed", name);
        }
    }

    let _ = run_command(&mount_restore);
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
