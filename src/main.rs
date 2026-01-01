mod apk;
mod commands;
mod constants;
mod device;
mod repo;
mod state;
mod util;

use std::env;
use std::fs;
use std::path::Path;
use std::process;

use apk::{generate_device_package, generate_remarkable_os_package, Apk};
use commands::{
    handle_add, handle_check_os, handle_del, handle_purge, handle_reenable,
    handle_self_uninstall, handle_testing, handle_upgrade,
};
use constants::VELLUM_ROOT;
use device::{get_apk_arch, get_device_type, get_os_version};
use repo::update_index;
use state::State;
use util::remove_glob;

struct AppState {
    os_mismatch: bool,
    os_cur: String,
    os_prev: String,
}

fn main() {
    let state = State::new(VELLUM_ROOT);
    let apk = Apk::new(VELLUM_ROOT);

    let app_state = ensure_remarkable_os(&state, &apk);
    ensure_device_package(&state, &apk);

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        show_help(&apk);
        return;
    }

    let cmd = &args[1];

    if app_state.os_mismatch && !is_allowed_during_mismatch(cmd) {
        println!();
        println!("OS upgraded ({} -> {}).", app_state.os_prev, app_state.os_cur);
        println!("Run 'vellum upgrade' to sync packages with new OS version.");
        println!();
        process::exit(1);
    }

    match cmd.as_str() {
        "--help" | "-h" => show_help(&apk),
        "install" => handle_add(&apk, &args[2..]),
        "remove" => handle_del(&apk, &args[2..]),
        "purge" => handle_purge(&apk, &args[2..]),
        "show" => handle_show(&apk, &args[2..]),
        "add" => handle_add(&apk, &args[2..]),
        "del" => handle_del(&apk, &args[2..]),
        "upgrade" => handle_upgrade(
            &state,
            &apk,
            &args[2..],
            app_state.os_mismatch,
            &app_state.os_prev,
            &app_state.os_cur,
        ),
        "reenable" => handle_reenable(),
        "check-os" => {
            if args.len() < 3 {
                eprintln!("Usage: vellum check-os <version>");
                eprintln!("Check if installed packages are compatible with a given OS version.");
                process::exit(1);
            }
            handle_check_os(&apk, &args[2]);
        }
        "self" => {
            if args.len() > 2 && args[2] == "uninstall" {
                handle_self_uninstall(&apk, VELLUM_ROOT, &args[3..]);
            } else {
                eprintln!("Unknown self command");
                eprintln!("Usage: vellum self uninstall [--all] [--yes]");
                process::exit(1);
            }
        }
        "testing" => handle_testing(VELLUM_ROOT, &args[2..]),
        _ => {
            let pass_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
            if let Err(e) = apk.exec(&pass_args) {
                eprintln!("exec error: {e}");
                process::exit(1);
            }
        }
    }
}

fn is_allowed_during_mismatch(cmd: &str) -> bool {
    !matches!(cmd, "add" | "install")
}

fn ensure_remarkable_os(state: &State, apk: &Apk) -> AppState {
    let os_cur = match get_os_version() {
        Ok(v) => v,
        Err(_) => {
            return AppState {
                os_mismatch: false,
                os_cur: String::new(),
                os_prev: String::new(),
            }
        }
    };

    let os_prev = state.get_os_version().unwrap_or_default();
    let arch = get_apk_arch();
    let repo_dir = format!("{VELLUM_ROOT}/local-repo/{arch}");
    let key_path = format!("{VELLUM_ROOT}/etc/apk/keys/local.rsa");

    if os_prev.is_empty() {
        if let Err(e) = fs::create_dir_all(&repo_dir) {
            eprintln!("warning: failed to create repo directory: {e}");
        }
        remove_glob(&format!("{repo_dir}/remarkable-os-*.apk"));
        if let Err(e) = generate_remarkable_os_package(&os_cur, &repo_dir, &key_path) {
            eprintln!("warning: failed to generate remarkable-os package: {e}");
        }
        if let Err(e) = update_index(&repo_dir, Some(&key_path)) {
            eprintln!("warning: failed to update local repo index: {e}");
        }
        if let Err(e) = state.set_os_version(&os_cur) {
            eprintln!("warning: failed to save OS version: {e}");
        }
        if let Err(e) = apk.run_silent(&["add", "remarkable-os"]) {
            eprintln!("warning: failed to register remarkable-os package: {e}");
        }

        AppState {
            os_mismatch: false,
            os_cur,
            os_prev: String::new(),
        }
    } else if os_cur != os_prev {
        AppState {
            os_mismatch: true,
            os_cur,
            os_prev,
        }
    } else {
        AppState {
            os_mismatch: false,
            os_cur,
            os_prev,
        }
    }
}

fn ensure_device_package(state: &State, apk: &Apk) {
    let Some(device_type) = get_device_type() else {
        return;
    };
    let prev_device = state.get_device().unwrap_or_default();
    let arch = get_apk_arch();
    let repo_dir = format!("{VELLUM_ROOT}/local-repo/{arch}");
    let key_path = format!("{VELLUM_ROOT}/etc/apk/keys/local.rsa");

    let pkg_path = format!("{repo_dir}/{device_type}-1.0.0-r0.apk");

    if device_type != prev_device || !Path::new(&pkg_path).exists() {
        if let Err(e) = fs::create_dir_all(&repo_dir) {
            eprintln!("warning: failed to create repo directory: {e}");
        }
        for d in &["rm1", "rm2", "rmpp", "rmppm"] {
            remove_glob(&format!("{repo_dir}/{d}-*.apk"));
        }
        if let Err(e) = generate_device_package(&device_type, &repo_dir, &key_path) {
            eprintln!("warning: failed to generate device package: {e}");
        }
        if let Err(e) = update_index(&repo_dir, Some(&key_path)) {
            eprintln!("warning: failed to update local repo index: {e}");
        }
        if let Err(e) = state.set_device(&device_type) {
            eprintln!("warning: failed to save device type: {e}");
        }
        if let Err(e) = apk.run_silent(&["add", &device_type]) {
            eprintln!("warning: failed to register device package: {e}");
        }
    }
}

fn show_help(apk: &Apk) {
    println!(
        r#"Vellum package manager for reMarkable

Usage: vellum <command> [options]

Vellum commands:
  upgrade             Upgrade packages (handles OS version changes)
  check-os <version>  Check package compatibility with an OS version
  reenable            Restore system files after OS upgrade
  testing             Manage testing repository (enable, disable, status)
  self uninstall      Remove vellum itself (--all to include packages)

Aliases:
  install <pkg>       Alias for 'add'
  remove <pkg>        Alias for 'del'
  purge <pkg>         Alias for 'del --purge'
  show <pkg>          Alias for 'info -a'
"#
    );
    let _ = apk.run(&["--help"]);
}

fn handle_show(apk: &Apk, args: &[String]) {
    let mut cmd_args = vec!["info", "-a"];
    cmd_args.extend(args.iter().map(|s| s.as_str()));

    if apk.run(&cmd_args).is_err() {
        process::exit(1);
    }
}

