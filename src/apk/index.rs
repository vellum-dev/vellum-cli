use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};

use anyhow::{anyhow, Result};
use flate2::bufread::MultiGzDecoder;
use tar::Archive;

use super::version::{version_gte, version_lt};

#[derive(Debug, Clone, Default)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub depends: Vec<String>,
}

impl Package {
    pub fn get_os_constraints(&self) -> (Option<String>, Option<String>) {
        let mut min_ver = None;
        let mut max_ver = None;

        for dep in &self.depends {
            if let Some(v) = dep.strip_prefix("remarkable-os>=") {
                min_ver = Some(v.to_string());
            } else if let Some(v) = dep.strip_prefix("remarkable-os<") {
                max_ver = Some(v.to_string());
            }
        }

        (min_ver, max_ver)
    }

    pub fn is_compatible_with_os(&self, os_version: &str) -> bool {
        let (min_ver, max_ver) = self.get_os_constraints();

        if min_ver.is_none() && max_ver.is_none() {
            return true;
        }

        if let Some(ref min) = min_ver {
            if !version_gte(os_version, min) {
                return false;
            }
        }

        if let Some(ref max) = max_ver {
            if !version_lt(os_version, max) {
                return false;
            }
        }

        true
    }
}

pub fn parse_index_file(path: &str) -> Result<Vec<Package>> {
    let f = File::open(path)?;
    parse_apkindex(BufReader::new(f))
}

pub fn parse_index_tar_gz(path: &str) -> Result<Vec<Package>> {
    let f = File::open(path)?;
    parse_index_from_tar_gz(f)
}

pub fn fetch_remote_index(repo_url: &str, arch: &str) -> Result<Vec<Package>> {
    let url = format!("{}/{}/APKINDEX.tar.gz", repo_url.trim_end_matches('/'), arch);

    eprintln!("[debug] Fetching: {}", url);

    let resp = match ureq::get(&url).call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[debug] HTTP request failed: {:?}", e);
            return Err(anyhow!("HTTP request failed: {}", e));
        }
    };

    eprintln!("[debug] HTTP status: {}", resp.status());

    if resp.status() != 200 {
        return Err(anyhow!("HTTP {}", resp.status()));
    }

    let mut data = Vec::new();
    resp.into_reader().read_to_end(&mut data)?;

    eprintln!("[debug] Downloaded {} bytes", data.len());

    parse_index_from_tar_gz(Cursor::new(data))
}

fn parse_index_from_tar_gz<R: Read>(reader: R) -> Result<Vec<Package>> {
    let mut data = Vec::new();
    let mut reader = reader;
    reader.read_to_end(&mut data)?;

    // Alpine's APKINDEX.tar.gz consists of multiple concatenated gzip streams:
    // 1. Signature segment (first gzip stream)
    // 2. Index tarball with DESCRIPTION and APKINDEX (second gzip stream)
    // MultiGzDecoder handles concatenated streams, unlike GzDecoder which stops after the first.
    let gz = MultiGzDecoder::new(Cursor::new(data));
    let mut archive = Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        if path.to_string_lossy() == "APKINDEX" {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            return parse_apkindex(BufReader::new(content.as_bytes()));
        }
    }

    Err(anyhow!("APKINDEX not found in archive"))
}

fn parse_apkindex<R: BufRead>(reader: R) -> Result<Vec<Package>> {
    let mut packages = Vec::new();
    let mut current = Package::default();

    for line in reader.lines() {
        let line = line?;

        if line.is_empty() {
            if !current.name.is_empty() {
                packages.push(current);
            }
            current = Package::default();
            continue;
        }

        if line.len() < 2 || line.as_bytes()[1] != b':' {
            continue;
        }

        let key = line.as_bytes()[0];
        let val = &line[2..];

        match key {
            b'P' => current.name = val.to_string(),
            b'V' => current.version = val.to_string(),
            b'D' => current.depends = val.split_whitespace().map(|s| s.to_string()).collect(),
            _ => {}
        }
    }

    if !current.name.is_empty() {
        packages.push(current);
    }

    Ok(packages)
}
