use std::fs;
use std::path::Path;

use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Builder, Header};

pub fn generate_remarkable_os_package(version: &str, repo_dir: &str) -> Result<()> {
    fs::create_dir_all(repo_dir)?;

    let pkginfo = format!(
        r#"pkgname = remarkable-os
pkgver = {}-r0
pkgdesc = Virtual package representing reMarkable OS version
url = https://github.com/vellum-dev/vellum-cli
arch = noarch
license = MIT
provides = /bin/sh
"#,
        version
    );

    let filename = format!("remarkable-os-{}-r0.apk", version);
    write_package(repo_dir, &filename, &pkginfo)
}

pub fn generate_device_package(device: &str, repo_dir: &str) -> Result<()> {
    fs::create_dir_all(repo_dir)?;

    let desc = match device {
        "rmpp" => "reMarkable Paper Pro",
        "rmppm" => "reMarkable Paper Pro Move",
        "rm2" => "reMarkable 2",
        "rm1" => "reMarkable 1",
        _ => "reMarkable Device",
    };

    let pkginfo = format!(
        r#"pkgname = {}
pkgver = 1.0.0-r0
pkgdesc = Virtual package for {}
url = https://github.com/vellum-dev/vellum-cli
arch = noarch
license = MIT
"#,
        device, desc
    );

    let filename = format!("{}-1.0.0-r0.apk", device);
    write_package(repo_dir, &filename, &pkginfo)
}

fn write_package(repo_dir: &str, filename: &str, pkginfo: &str) -> Result<()> {
    let mut data_buf = Vec::new();
    {
        let gz = GzEncoder::new(&mut data_buf, Compression::default());
        let mut tar = Builder::new(gz);

        let mut header = Header::new_gnu();
        header.set_path("./")?;
        header.set_mode(0o755);
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        tar.append(&header, &[] as &[u8])?;

        tar.into_inner()?.finish()?;
    }

    let mut apk_buf = Vec::new();
    {
        let gz = GzEncoder::new(&mut apk_buf, Compression::default());
        let mut tar = Builder::new(gz);

        let pkginfo_bytes = pkginfo.as_bytes();
        let mut header = Header::new_gnu();
        header.set_path(".PKGINFO")?;
        header.set_mode(0o644);
        header.set_size(pkginfo_bytes.len() as u64);
        header.set_cksum();
        tar.append(&header, pkginfo_bytes)?;

        let mut header = Header::new_gnu();
        header.set_path("data.tar.gz")?;
        header.set_mode(0o644);
        header.set_size(data_buf.len() as u64);
        header.set_cksum();
        tar.append(&header, data_buf.as_slice())?;

        tar.into_inner()?.finish()?;
    }

    let output_path = Path::new(repo_dir).join(filename);
    fs::write(output_path, &apk_buf)?;

    Ok(())
}
