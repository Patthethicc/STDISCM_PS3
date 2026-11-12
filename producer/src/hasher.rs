//! SHA-256 of a file, used for duplicate detection.

use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

const BUF_SIZE: usize = 64 * 1024;

/// Hex-encoded SHA-256 of the file, computed by streaming (constant memory).
pub fn sha256_hex(path: &Path) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
