use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{anyhow, Result};
use base64::prelude::*;
use flate2::bufread::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use pkcs1::DecodeRsaPrivateKey;
use pkcs8::DecodePrivateKey;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::RsaPrivateKey;
use sha1::{Digest, Sha1};
use tar::{Archive, Builder, Header};

pub fn update_index(repo_dir: &str, key_path: Option<&str>) -> Result<()> {
    let apks: Vec<_> = fs::read_dir(repo_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "apk")
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();

    let mut index_buf = Vec::new();
    for apk_path in &apks {
        append_package_to_index(&mut index_buf, apk_path)?;
    }

    let mut unsigned_buf = Vec::new();
    {
        let mut gz = GzEncoder::new(&mut unsigned_buf, Compression::default());
        {
            let mut tar = Builder::new(&mut gz);

            let mut header = Header::new_gnu();
            header.set_path("APKINDEX")?;
            header.set_mode(0o644);
            header.set_size(index_buf.len() as u64);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            tar.append(&header, index_buf.as_slice())?;

            tar.finish()?;
        }
        gz.finish()?;
    }

    let output_path = Path::new(repo_dir).join("APKINDEX.tar.gz");

    if let Some(key_path) = key_path {
        if let Ok(key_data) = fs::read_to_string(key_path) {
            return write_signed_index(&output_path, &unsigned_buf, &key_data);
        }
    }

    fs::write(output_path, &unsigned_buf)?;
    Ok(())
}

fn append_package_to_index(w: &mut Vec<u8>, apk_path: &Path) -> Result<()> {
    let data = fs::read(apk_path)?;
    let size = data.len();

    let mut hasher = Sha1::new();
    hasher.update(&data);
    let cksum = BASE64_STANDARD.encode(hasher.finalize());

    // APK files are concatenated gzip streams: signature, control (.PKGINFO), data
    let gz = MultiGzDecoder::new(Cursor::new(&data));
    let mut archive = Archive::new(gz);

    let mut pkginfo = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();
        if path == ".PKGINFO" {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            pkginfo = Some(content);
            break;
        }
    }

    let pkginfo = pkginfo.ok_or_else(|| anyhow!("no .PKGINFO found"))?;

    writeln!(w, "C:Q1{}", cksum)?;

    for line in pkginfo.lines() {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            continue;
        }
        let key = parts[0].trim();
        let val = parts[1].trim();
        match key {
            "pkgname" => writeln!(w, "P:{}", val)?,
            "pkgver" => writeln!(w, "V:{}", val)?,
            "arch" => writeln!(w, "A:{}", val)?,
            "pkgdesc" => writeln!(w, "T:{}", val)?,
            "url" => writeln!(w, "U:{}", val)?,
            "license" => writeln!(w, "L:{}", val)?,
            "provides" => writeln!(w, "p:{}", val)?,
            "depends" => writeln!(w, "D:{}", val)?,
            _ => {}
        }
    }

    writeln!(w, "S:{}", size)?;
    writeln!(w, "I:0")?;
    writeln!(w)?;

    Ok(())
}

fn write_signed_index(output_path: &Path, unsigned_data: &[u8], key_pem: &str) -> Result<()> {
    let key = RsaPrivateKey::from_pkcs1_pem(key_pem)
        .or_else(|_| RsaPrivateKey::from_pkcs8_pem(key_pem))
        .map_err(|e| anyhow!("failed to parse private key: {}", e))?;

    let mut hasher = Sha1::new();
    hasher.update(unsigned_data);
    let digest = hasher.finalize();

    let padding = Pkcs1v15Sign::new::<Sha1>();
    let signature = key.sign(padding, &digest)?;

    let mut sig_tar_buf = Vec::new();
    {
        let mut tar = Builder::new(&mut sig_tar_buf);

        let mut header = Header::new_gnu();
        header.set_path(".SIGN.RSA.local.rsa.pub")?;
        header.set_mode(0o644);
        header.set_size(signature.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar.append(&header, signature.as_slice())?;

        tar.finish()?;
    }

    let mut sig_gz_buf = Vec::new();
    {
        // Strip the last 1024 bytes (2 blocks of zeros) added by finish()
        // This ensures we have a valid tar stream without the EOF markers,
        // allowing concatenation with the next stream.
        let sig_len = sig_tar_buf.len();
        let sig_tar_data = if sig_len > 1024 {
            &sig_tar_buf[..sig_len - 1024]
        } else {
            &sig_tar_buf
        };

        let mut gz = GzEncoder::new(&mut sig_gz_buf, Compression::best());
        gz.write_all(sig_tar_data)?;
        gz.finish()?;
    }

    let mut final_buf = sig_gz_buf;
    final_buf.extend_from_slice(unsigned_data);

    fs::write(output_path, &final_buf)?;
    Ok(())
}
