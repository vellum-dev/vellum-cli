use std::fs;
use std::process;

use anyhow::Result;

const TESTING_REPO_URL: &str = "https://packages.vellum.delivery/testing";
const TESTING_TAG: &str = "@testing";

pub struct TestingManager {
    repos_path: String,
}

impl TestingManager {
    pub fn new(vellum_root: &str) -> Self {
        Self {
            repos_path: format!("{vellum_root}/etc/apk/repositories"),
        }
    }

    pub fn is_enabled(&self) -> bool {
        let content = match fs::read_to_string(&self.repos_path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        content.lines().any(|line| line.trim().starts_with(TESTING_TAG))
    }

    pub fn enable(&self) -> Result<()> {
        let content = fs::read_to_string(&self.repos_path)?;
        let lines: Vec<&str> = content.lines().collect();

        for line in &lines {
            if line.trim().starts_with(TESTING_TAG) {
                return Ok(());
            }
        }

        let testing_line = format!("{TESTING_TAG} {TESTING_REPO_URL}");
        let mut new_lines = Vec::new();
        let mut inserted = false;

        for line in &lines {
            new_lines.push(line.to_string());
            if !inserted && line.contains("local-repo") {
                new_lines.push(testing_line.clone());
                inserted = true;
            }
        }

        if !inserted {
            new_lines.insert(0, testing_line);
        }

        fs::write(&self.repos_path, new_lines.join("\n"))?;
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        let content = fs::read_to_string(&self.repos_path)?;

        let new_lines: Vec<&str> = content
            .lines()
            .filter(|line| !line.trim().starts_with(TESTING_TAG))
            .collect();

        fs::write(&self.repos_path, new_lines.join("\n"))?;
        Ok(())
    }
}

pub fn handle_testing(vellum_root: &str, args: &[String]) {
    let mgr = TestingManager::new(vellum_root);

    if args.is_empty() {
        if mgr.is_enabled() {
            println!("Testing repository: enabled");
        } else {
            println!("Testing repository: disabled");
        }
        println!();
        println!("Usage: vellum testing <enable|disable|status>");
        return;
    }

    match args[0].as_str() {
        "enable" => {
            if mgr.is_enabled() {
                println!("Testing repository is already enabled.");
                return;
            }
            if let Err(e) = mgr.enable() {
                eprintln!("Error enabling testing repository: {e}");
                process::exit(1);
            }
            println!("Testing repository enabled.");
            println!("Run 'vellum update' to refresh the package index.");
            println!();
            println!("Install testing packages with: vellum add <package>@testing");
        }
        "disable" => {
            if !mgr.is_enabled() {
                println!("Testing repository is already disabled.");
                return;
            }
            if let Err(e) = mgr.disable() {
                eprintln!("Error disabling testing repository: {e}");
                process::exit(1);
            }
            println!("Testing repository disabled.");
            println!("Run 'vellum update' to refresh the package index.");
        }
        "status" => {
            if mgr.is_enabled() {
                println!("Testing repository: enabled");
            } else {
                println!("Testing repository: disabled");
            }
        }
        cmd => {
            eprintln!("Unknown testing command: {cmd}");
            println!("Usage: vellum testing <enable|disable|status>");
            process::exit(1);
        }
    }
}
