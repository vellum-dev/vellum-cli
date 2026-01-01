use std::process;

use crate::apk::Apk;

pub fn handle_add(apk: &Apk, args: &[String]) {
    for arg in args {
        if arg == "vellum" {
            eprintln!("Error: Cannot add/remove vellum package directly.");
            eprintln!("Use 'vellum self uninstall' to remove vellum.");
            process::exit(1);
        }
    }

    let mut cmd_args = vec!["add", "--cache-predownload"];
    cmd_args.extend(args.iter().map(|s| s.as_str()));

    let result = apk.run(&cmd_args);
    let _ = apk.cache_purge();

    if result.is_err() {
        process::exit(1);
    }
}
