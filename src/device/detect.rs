use std::fs;
use std::process::Command;

use anyhow::{anyhow, Result};

fn parse_version(data: &str, key: &str) -> Option<String> {
    data.lines()
        .filter_map(|line| line.strip_prefix(key))
        .map(|ver| ver.trim_matches(|c| c == '"' || c == '\''))
        .find(|ver| !ver.is_empty())
        .map(|ver| ver.to_string())
}

pub fn get_os_version() -> Result<String> {
    if let Ok(data) = fs::read_to_string("/usr/share/remarkable/update.conf") {
        if let Some(ver) = parse_version(&data, "REMARKABLE_RELEASE_VERSION=") {
            return Ok(ver);
        }
    }

    if let Ok(data) = fs::read_to_string("/etc/os-release") {
        if let Some(ver) = parse_version(&data, "IMG_VERSION=") {
            return Ok(ver);
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
    ("Chiappa", "rmppmove"),
    ("Tatsu", "rmppure"),
    ("reMarkable 1.0", "rm1"),
    ("reMarkable 2.0", "rm2"),
];

pub fn get_device_type() -> Option<String> {
    let data = fs::read_to_string("/sys/devices/soc0/machine").ok()?;
    let machine = data.trim();
    DEVICE_PATTERNS
        .iter()
        .find(|(pattern, _)| machine.contains(pattern))
        .map(|(_, device)| device.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATE_CONF_3_3: &str = "[General]\n\
        #REMARKABLE_RELEASE_APPID={98DA7DF2-4E3E-4744-9DE6-EC931886ABAB}\n\
        #SERVER=https://updates.cloud.remarkable.engineering/service/update2\n\
        #GROUP=Prod\n\
        #PLATFORM=reMarkable2\n\
        REMARKABLE_RELEASE_VERSION=3.3.2.1666\n";

    const OS_RELEASE_3_27: &str = "ID=codex\n\
        NAME=\"Codex Linux\"\n\
        VERSION=\"5.7.126 (scarthgap)\"\n\
        VERSION_ID=5.7.126\n\
        BUILD_MODE_RM=\"public\"\n\
        IMG_VERSION=\"3.27.3.0\"\n";

    const OS_RELEASE_3_3: &str = "ID=codex\n\
        NAME=\"Codex Linux\"\n\
        VERSION=\"3.1.158-4 (dunfell)\"\n\
        VERSION_ID=3.1.158-4\n\
        DISTRO_CODENAME=\"dunfell\"\n";

    #[test]
    fn reads_release_version_from_update_conf() {
        let ver = parse_version(UPDATE_CONF_3_3, "REMARKABLE_RELEASE_VERSION=");
        assert_eq!(ver.as_deref(), Some("3.3.2.1666"));
    }

    #[test]
    fn reads_img_version_from_os_release() {
        let ver = parse_version(OS_RELEASE_3_27, "IMG_VERSION=");
        assert_eq!(ver.as_deref(), Some("3.27.3.0"));
    }

    #[test]
    fn ignores_commented_appid_key() {
        let ver = parse_version(UPDATE_CONF_3_3, "REMARKABLE_RELEASE_APPID=");
        assert_eq!(ver, None);
    }

    #[test]
    fn older_os_release_has_no_img_version() {
        assert_eq!(parse_version(OS_RELEASE_3_3, "IMG_VERSION="), None);
    }
}
