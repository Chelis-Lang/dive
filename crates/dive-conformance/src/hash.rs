//! SHA-256 helpers.

use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

/// Lower-case hex encoding of `bytes`.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // Writing to a String cannot fail.
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// SHA-256 of `bytes`, as lower-case hex.
#[must_use]
pub fn sha256_bytes(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

/// SHA-256 of the file at `path`, as lower-case hex.
///
/// # Errors
/// Fails if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| Error::io(path, &e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::io(path, &e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(&hasher.finalize()))
}

/// Verifies that the file at `path` has SHA-256 `expected`. On a mismatch the
/// file is deleted and [`Error::HashMismatch`] is returned.
///
/// # Errors
/// Fails if the file cannot be read, or on a mismatch.
pub fn verify_file(path: &Path, expected: &str, what: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if actual == expected {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| Error::io(path, &e))?;
    Err(Error::HashMismatch {
        what: what.to_owned(),
        expected: expected.to_owned(),
        actual,
    })
}

/// Whether `s` is a lower-case hex SHA-256 digest.
#[must_use]
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_digest() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hex_validation() {
        assert!(is_sha256_hex(&sha256_bytes(b"")));
        assert!(!is_sha256_hex("ABC"));
        assert!(!is_sha256_hex(&"A".repeat(64)));
    }

    #[test]
    fn mismatch_deletes_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("hash-test")?;
        let path = dir.path().join("f");
        std::fs::write(&path, b"corrupt")?;
        let err = verify_file(&path, &sha256_bytes(b"abc"), "test file");
        assert!(matches!(err, Err(Error::HashMismatch { .. })));
        assert!(!path.exists());
        Ok(())
    }
}
