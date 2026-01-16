use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::Result;

pub struct Apk {
    root: PathBuf,
}

impl Apk {
    pub fn new(vellum_root: &str) -> Self {
        Self {
            root: PathBuf::from(vellum_root),
        }
    }

    fn bin_path(&self) -> PathBuf {
        self.root.join("bin").join("apk.vellum")
    }

    fn base_args(&self) -> Vec<String> {
        vec![
            "--root".to_string(),
            self.root.to_string_lossy().to_string(),
            "--install-root".to_string(),
            "/".to_string(),
            "--no-logfile".to_string(),
        ]
    }

    pub fn run(&self, args: &[&str]) -> Result<()> {
        let mut cmd_args = self.base_args();
        cmd_args.extend(args.iter().map(|s| s.to_string()));

        let status = Command::new(self.bin_path())
            .args(&cmd_args)
            .env("APK_CONFIG", self.root.join("etc").join("apk").join("config"))
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "apk exited with code {}",
                status.code().unwrap_or(-1)
            ))
        }
    }

    pub fn run_silent(&self, args: &[&str]) -> Result<()> {
        let mut cmd_args = self.base_args();
        cmd_args.extend(args.iter().map(|s| s.to_string()));

        let status = Command::new(self.bin_path())
            .args(&cmd_args)
            .env("APK_CONFIG", self.root.join("etc").join("apk").join("config"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "apk exited with code {}",
                status.code().unwrap_or(-1)
            ))
        }
    }

    pub fn output(&self, args: &[&str]) -> Result<String> {
        let mut cmd_args = self.base_args();
        cmd_args.extend(args.iter().map(|s| s.to_string()));

        let output = Command::new(self.bin_path())
            .args(&cmd_args)
            .env("APK_CONFIG", self.root.join("etc").join("apk").join("config"))
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn exec(&self, args: &[&str]) -> Result<()> {
        let mut cmd_args = self.base_args();
        cmd_args.extend(args.iter().map(|s| s.to_string()));

        let err = Command::new(self.bin_path())
            .args(&cmd_args)
            .env("APK_CONFIG", self.root.join("etc").join("apk").join("config"))
            .exec();

        Err(anyhow::anyhow!("exec failed: {err}"))
    }

    pub fn list_installed(&self) -> Result<Vec<String>> {
        let out = self.output(&["info", "-q"])?;
        if out.is_empty() {
            return Ok(Vec::new());
        }
        Ok(out.lines().map(|s| s.to_string()).collect())
    }

    pub fn get_dependencies(&self, pkg: &str) -> Result<Vec<String>> {
        let out = self.output(&["info", "-R", pkg])?;
        Ok(out.lines().map(|s| s.to_string()).collect())
    }

    pub fn get_package_version(&self, pkg: &str) -> Result<Option<String>> {
        let out = self.output(&["list", "-I", pkg])?;
        if out.is_empty() {
            return Ok(None);
        }
        let first_field = out
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or("");
        let prefix = format!("{}-", pkg);
        if let Some(rest) = first_field.strip_prefix(&prefix) {
            if let Some((ver, _)) = rest.rsplit_once("-r") {
                return Ok(Some(ver.to_string()));
            }
        }
        Ok(None)
    }

    pub fn cache_purge(&self) -> Result<()> {
        self.run_silent(&["cache", "purge"])
    }
}
