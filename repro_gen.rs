use std::fs;
use std::io::{Read, Write};
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Builder, Header};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create a dummy APKINDEX (unsigned)
    let index_content = "P:test-package\nV:1.0.0\n";
    let mut unsigned_buf = Vec::new();
    {
        let mut gz = GzEncoder::new(&mut unsigned_buf, Compression::default());
        {
            let mut tar = Builder::new(&mut gz);
            let mut header = Header::new_ustar();
            header.set_path("APKINDEX")?;
            header.set_mode(0o644);
            header.set_size(index_content.len() as u64);
            header.set_cksum();
            tar.append(&header, index_content.as_bytes())?;
            tar.finish()?;
        }
        gz.finish()?;
    }

    // 2. Create a dummy signature
    let signature = vec![0u8; 256]; // Dummy 256-byte signature

    // 3. Create the signature tarball (Truncated logic)
    let mut sig_tar_buf = Vec::new();
    {
        let mut tar = Builder::new(&mut sig_tar_buf);
        let mut header = Header::new_ustar();
        header.set_path(".SIGN.RSA.local.rsa.pub")?;
        header.set_mode(0o644);
        header.set_size(signature.len() as u64);
        header.set_cksum();
        tar.append(&header, signature.as_slice())?;
        tar.finish()?;
    }

    // Calculate blocks
    let content_blocks = (512 + signature.len() + 511) / 512;
    let sig_tar_data = &sig_tar_buf[..content_blocks * 512];
    
    println!("Signature len: {}", signature.len());
    println!("Content blocks: {}", content_blocks);
    println!("Sliced data len: {}", sig_tar_data.len());
    println!("Original tar buf len: {}", sig_tar_buf.len());

    // 4. Gzip the signature
    let mut sig_gz_buf = Vec::new();
    {
        let mut gz = GzEncoder::new(&mut sig_gz_buf, Compression::best());
        gz.write_all(sig_tar_data)?;
        gz.finish()?;
    }

    // 5. Concatenate
    let mut final_buf = sig_gz_buf;
    final_buf.extend_from_slice(&unsigned_buf);

    fs::write("test_index.tar.gz", &final_buf)?;
    println!("Created test_index.tar.gz");

    Ok(())
}
