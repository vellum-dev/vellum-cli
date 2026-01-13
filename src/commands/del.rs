use std::process;

use crate::apk::Apk;

pub fn handle_del(apk: &Apk, args: &[String]) {
    for arg in args {
        if arg == "vellum" {
            eprintln!("Error: Cannot add/remove vellum package directly.");
            eprintln!("Use 'vellum self uninstall' to remove vellum.");
            process::exit(1);
        }
    }

    let mut cmd_args = vec!["del"];
    cmd_args.extend(args.iter().map(|s| s.as_str()));

    if apk.run(&cmd_args).is_err() {
        process::exit(1);
    }
}

pub fn handle_purge(apk: &Apk, args: &[String]) {
    for arg in args {
        if arg == "vellum" {
            eprintln!("Error: Cannot add/remove vellum package directly.");
            eprintln!("Use 'vellum self uninstall' to remove vellum.");
            process::exit(1);
        }
    }

    std::env::set_var("VELLUM_PURGE", "1");

    let mut cmd_args = vec!["del", "--purge", "--preserve-env"];
    cmd_args.extend(args.iter().map(|s| s.as_str()));

    if apk.run(&cmd_args).is_err() {
        process::exit(1);
    }
}
