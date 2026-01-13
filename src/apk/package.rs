use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use pkcs1::DecodeRsaPrivateKey;
use pkcs8::DecodePrivateKey;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::RsaPrivateKey;
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest as Sha256Digest, Sha256};
use tar::{Builder, Header};

pub fn generate_remarkable_os_package(version: &str, repo_dir: &str, key_path: &str) -> Result<()> {
    fs::create_dir_all(repo_dir)?;

    let pkginfo = format!(
        r#"pkgname = remarkable-os
pkgver = {version}-r0
pkgdesc = Virtual package representing reMarkable OS version
url = https://github.com/vellum-dev/vellum-cli
arch = noarch
license = MIT
provides = /bin/sh
"#
    );

    let filename = format!("remarkable-os-{version}-r0.apk");
    write_package(repo_dir, &filename, &pkginfo, key_path)
}

pub fn generate_device_package(device: &str, repo_dir: &str, key_path: &str) -> Result<()> {
    fs::create_dir_all(repo_dir)?;

    let desc = match device {
        "rmpp" => "reMarkable Paper Pro",
        "rmppm" => "reMarkable Paper Pro Move",
        "rm2" => "reMarkable 2",
        "rm1" => "reMarkable 1",
        _ => "reMarkable Device",
    };

    let pkginfo = format!(
        r#"pkgname = {device}
pkgver = 1.0.0-r0
pkgdesc = Virtual package for {desc}
url = https://github.com/vellum-dev/vellum-cli
arch = noarch
license = MIT
"#
    );

    let filename = format!("{device}-1.0.0-r0.apk");
    write_package(repo_dir, &filename, &pkginfo, key_path)
}

fn write_package(repo_dir: &str, filename: &str, pkginfo: &str, key_path: &str) -> Result<()> {
    // v2 APK format: concatenated gzip streams
    // Stream 1: Signature (tar with .SIGN.RSA.*)
    // Stream 2: Control section (tar containing .PKGINFO with datahash)
    // Stream 3: Data section (tar with actual files)

    // Build data section first (empty for virtual packages)
    // We need this first to compute datahash for .PKGINFO
    let mut data_buf = Vec::new();
    {
        let gz = GzEncoder::new(&mut data_buf, Compression::default());
        let tar = Builder::new(gz);
        tar.into_inner()?.finish()?;
    }

    // Compute datahash (SHA256 of data section)
    let mut sha256 = Sha256::new();
    Sha256Digest::update(&mut sha256, &data_buf);
    let datahash = format!("{:x}", sha256.finalize());

    // Add datahash to pkginfo
    let pkginfo_with_hash = format!("{pkginfo}datahash = {datahash}\n");

    // Build control section with updated pkginfo
    let mut control_buf = Vec::new();
    {
        let gz = GzEncoder::new(&mut control_buf, Compression::default());
        let mut tar = Builder::new(gz);

        let pkginfo_bytes = pkginfo_with_hash.as_bytes();
        let mut header = Header::new_ustar();
        header.set_path(".PKGINFO")?;
        header.set_mode(0o644);
        header.set_size(pkginfo_bytes.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar.append(&header, pkginfo_bytes)?;

        tar.into_inner()?.finish()?;
    }

    // Sign the control section
    let key_data = fs::read_to_string(key_path)?;
    let key = RsaPrivateKey::from_pkcs1_pem(&key_data)
        .or_else(|_| RsaPrivateKey::from_pkcs8_pem(&key_data))
        .map_err(|e| anyhow!("failed to parse private key: {e}"))?;

    let mut hasher = Sha1::new();
    Sha1Digest::update(&mut hasher, &control_buf);
    let digest = hasher.finalize();

    let padding = Pkcs1v15Sign::new::<Sha1>();
    let signature = key.sign(padding, &digest)?;

    // Build signature section
    let mut sig_tar_buf = Vec::new();
    {
        let mut tar = Builder::new(&mut sig_tar_buf);

        let mut header = Header::new_ustar();
        header.set_path(".SIGN.RSA.local.rsa.pub")?;
        header.set_mode(0o644);
        header.set_size(signature.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar.append(&header, signature.as_slice())?;

        tar.finish()?;
    }

    // Gzip the signature tar (strip EOF markers for concatenation)
    let mut sig_gz_buf = Vec::new();
    {
        let sig_len = sig_tar_buf.len();
        let sig_tar_data = if sig_len > 1024 {
            &sig_tar_buf[..sig_len - 1024]
        } else {
            &sig_tar_buf
        };

        let mut gz = GzEncoder::new(&mut sig_gz_buf, Compression::default());
        gz.write_all(sig_tar_data)?;
        gz.finish()?;
    }

    // Concatenate: signature + control + data
    let mut apk_buf = sig_gz_buf;
    apk_buf.extend_from_slice(&control_buf);
    apk_buf.extend_from_slice(&data_buf);

    let output_path = Path::new(repo_dir).join(filename);
    fs::write(output_path, &apk_buf)?;

    Ok(())
}
