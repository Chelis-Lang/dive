//! The harness error type.

use std::fmt;
use std::path::Path;

/// A harness failure. Every variant is a hard error: the harness never turns
/// one of these into a verdict.
#[derive(Debug)]
pub enum Error {
    /// A configuration file (`oracles.toml`, `corpora.toml`, a manifest or an
    /// adapter description) is missing, malformed or inconsistent.
    Config(String),
    /// A downloaded or cached artifact does not match its pinned SHA-256. The
    /// offending file has already been deleted.
    HashMismatch {
        /// What was being verified.
        what: String,
        /// The pinned digest.
        expected: String,
        /// The digest of the bytes on disk.
        actual: String,
    },
    /// A file-system operation failed.
    Io(String),
    /// An external command (`curl`, `git`, `tar`, `java`) failed.
    Command(String),
    /// The command line was malformed.
    Usage(String),
    /// A golden file exists and `--reason` was not given, or a golden is
    /// otherwise unusable.
    Golden(String),
}

impl Error {
    /// Wraps an I/O error with the path it concerns.
    #[must_use]
    pub fn io(path: &Path, err: &std::io::Error) -> Self {
        Self::Io(format!("{}: {err}", path.display()))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(m) => write!(f, "configuration error: {m}"),
            Self::HashMismatch {
                what,
                expected,
                actual,
            } => write!(
                f,
                "SHA-256 mismatch for {what}: expected {expected}, got {actual}; the file was deleted"
            ),
            Self::Io(m) => write!(f, "I/O error: {m}"),
            Self::Command(m) => write!(f, "command failed: {m}"),
            Self::Usage(m) => write!(f, "usage error: {m}"),
            Self::Golden(m) => write!(f, "golden error: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// The harness result type.
pub type Result<T> = std::result::Result<T, Error>;
