//! EVBANK: the Energy Voice Bank format (`.evbank`, a zip with a `manifest.json`).
//!
//! A voice bank is a container of data. Nothing in it is ever executed. The modules:
//! - `manifest`: the manifest types and their validation (the rules of `Documentation/EVBANK_SCHEMA.md`)
//! - `safety`: path rules and size limits used for every file that comes from outside
//! - `archive`: safe extraction of archives and folders, and packing
//! - `verify`: checks an unpacked bank (file set, checksums, WAV headers, model files)
//! - `store`: the installed-bank store (atomic install, list, verify, uninstall, export)

pub mod archive;
pub mod manifest;
pub mod safety;
pub mod store;
pub mod verify;
mod wav;

#[cfg(test)]
mod tests;

use std::fmt;

/// Everything that can go wrong while reading, checking or installing a bank. `Display` is the user-facing text.
#[derive(Debug)]
pub enum EvbankError {
    /// The file is not a usable archive or folder (not a zip, no manifest, ...).
    NotABank(String),
    /// The manifest or the file set breaks the format rules. One line per problem.
    Invalid(Vec<String>),
    /// A file's content does not match its checksum, or the bank is damaged.
    Corrupted(Vec<String>),
    /// Something dangerous: path traversal, zip bomb, links, ...
    Unsafe(String),
    /// The bank uses a newer format version than this app understands.
    UnsupportedVersion { found: u32, supported: u32 },
    /// A bank with this id is installed already (retry with `replace` to overwrite it).
    AlreadyInstalled { id: String, installed: String, incoming: String },
    NotInstalled(String),
    Io(String),
}

impl fmt::Display for EvbankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABank(msg) => write!(f, "Not a voice bank: {msg}"),
            Self::Invalid(list) => write!(f, "Invalid voice bank:\n- {}", list.join("\n- ")),
            Self::Corrupted(list) => write!(f, "Corrupted voice bank:\n- {}", list.join("\n- ")),
            Self::Unsafe(msg) => write!(f, "Rejected for safety: {msg}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "This voice bank uses format version {found}, but this app supports up to version {supported}. Update Energy Voice Editor."
            ),
            Self::AlreadyInstalled { id, installed, incoming } => {
                write!(f, "Voice bank '{id}' is already installed (version {installed}); the new one is version {incoming}.")
            }
            Self::NotInstalled(id) => write!(f, "Voice bank '{id}' is not installed."),
            Self::Io(msg) => write!(f, "File error: {msg}"),
        }
    }
}

impl std::error::Error for EvbankError {}

impl From<std::io::Error> for EvbankError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EvbankError>;
