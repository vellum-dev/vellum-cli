use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

use crate::apk::Apk;
use crate::constants::VIRTUAL_PKGS;

pub fn handle_self_uninstall(apk: &Apk, vellum_root: &str, args: &[String]) {
    let mut uninstall_all = false;
    let mut uninstall_yes = false;

    for arg in args {
        match arg.as_str() {
            "--all" => uninstall_all = true,
            "--yes" | "-y" => uninstall_yes = true,
            _ => {}
        }
    }

    if !uninstall_yes {
        let msg = if uninstall_all {
            "This will remove vellum and permanently delete all installed packages and their data"
        } else {
            "This will remove vellum"
        };
        print!("{msg}. Continue? [y/N] ");
        let _ = io::stdout().flush();

        let stdin = io::stdin();
        let mut line = String::new();
        let _ = stdin.lock().read_line(&mut line);
        let confirm = line.trim().to_lowercase();

        if confirm != "y" && confirm != "yes" {
            println!("Aborted.");
            process::exit(1);
        }
    }

    if uninstall_all {
        println!("Removing all installed packages...");
        env::set_var("VELLUM_PURGE", "1");
        if let Ok(installed) = apk.list_installed() {
            for pkg in installed {
                if pkg == "vellum" || VIRTUAL_PKGS.contains(&pkg.as_str()) {
                    continue;
                }
                if let Err(e) = apk.run_silent(&["del", "--purge", "--preserve-env", &pkg]) {
                    eprintln!("warning: failed to remove {pkg}: {e}");
                }
            }
        }
    }

    println!("Removing vellum...");

    if let Ok(home) = env::var("HOME") {
        let bashrc = format!("{home}/.bashrc");
        if let Ok(content) = fs::read_to_string(&bashrc) {
            let new_lines: Vec<&str> = content
                .lines()
                .filter(|line| !line.contains(".vellum"))
                .collect();
            if let Err(e) = fs::write(&bashrc, new_lines.join("\n")) {
                eprintln!("warning: failed to update {bashrc}: {e}");
            }
        }
    }

    if let Err(e) = fs::remove_dir_all(vellum_root) {
        eprintln!("warning: failed to remove {vellum_root}: {e}");
    }
    println!("Vellum has been removed.");
}
