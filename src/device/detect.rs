use std::fs;
use std::process::Command;

use anyhow::{anyhow, Result};

pub fn get_os_version() -> Result<String> {
    if let Ok(data) = fs::read_to_string("/usr/share/remarkable/update.conf") {
        for line in data.lines() {
            if let Some(ver) = line.strip_prefix("RELEASE_VERSION=") {
                let ver = ver.trim_matches(|c| c == '"' || c == '\'');
                if !ver.is_empty() {
                    return Ok(ver.to_string());
                }
            }
        }
    }

    if let Ok(data) = fs::read_to_string("/etc/os-release") {
        for line in data.lines() {
            if let Some(ver) = line.strip_prefix("IMG_VERSION=") {
                let ver = ver.trim_matches(|c| c == '"' || c == '\'');
                if !ver.is_empty() {
                    return Ok(ver.to_string());
                }
            }
        }
    }

    Err(anyhow!("could not detect OS version"))
}

pub fn get_apk_arch() -> String {
    if cfg!(target_arch = "aarch64") {
        return "aarch64".to_string();
    }

    if let Ok(output) = Command::new("uname").arg("-m").output() {
        let m = String::from_utf8_lossy(&output.stdout).trim().to_string();
        match m.as_str() {
            "aarch64" => return "aarch64".to_string(),
            "armv7l" => return "armv7".to_string(),
            _ => {}
        }
    }
    "noarch".to_string()
}

const DEVICE_PATTERNS: &[(&str, &str)] = &[
    ("Ferrari", "rmpp"),
    ("Chiappa", "rmppm"),
    ("reMarkable 1.0", "rm1"),
    ("reMarkable 2.0", "rm2"),
];

pub fn get_device_type() -> Option<String> {
    let data = fs::read_to_string("/sys/devices/soc0/machine").ok()?;
    let machine = data.trim();
    DEVICE_PATTERNS
        .iter()
        .find(|(pattern, _)| machine == *pattern)
        .map(|(_, device)| device.to_string())
}
