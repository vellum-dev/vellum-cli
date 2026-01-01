use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use pkcs1::DecodeRsaPrivateKey;
use pkcs8::DecodePrivateKey;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::RsaPrivateKey;
use sha1::{Digest, Sha1};
use tar::{Builder, Header};

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

    if apks.is_empty() {
        return Err(anyhow!("no .apk files found in {repo_dir}"));
    }

    let output_path = Path::new(repo_dir).join("APKINDEX.tar.gz");
    let temp_path = Path::new(repo_dir).join(".APKINDEX.unsigned.tar.gz");

    let vellum_root = Path::new(repo_dir)
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow!("could not determine vellum root"))?;

    let apk_bin = vellum_root.join("bin").join("apk.vellum");
    let keys_dir = vellum_root.join("etc").join("apk").join("keys");

    let mut cmd = Command::new(&apk_bin);
    cmd.arg("index")
        .arg("--keys-dir")
        .arg(&keys_dir)
        .arg("-o")
        .arg(&temp_path)
        .args(apks.iter().map(|p| p.as_os_str()));

    let status = cmd.status()?;
    if !status.success() {
        return Err(anyhow!(
            "apk index failed with code {}",
            status.code().unwrap_or(-1)
        ));
    }

    let unsigned_buf = fs::read(&temp_path)?;
    let _ = fs::remove_file(&temp_path);

    if let Some(key_path) = key_path {
        if let Ok(key_data) = fs::read_to_string(key_path) {
            return write_signed_index(&output_path, &unsigned_buf, &key_data);
        }
    }

    fs::write(output_path, &unsigned_buf)?;
    Ok(())
}

fn write_signed_index(output_path: &Path, unsigned_data: &[u8], key_pem: &str) -> Result<()> {
    let key = RsaPrivateKey::from_pkcs1_pem(key_pem)
        .or_else(|_| RsaPrivateKey::from_pkcs8_pem(key_pem))
        .map_err(|e| anyhow!("failed to parse private key: {e}"))?;

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
