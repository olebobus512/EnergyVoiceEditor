//! Checks an unpacked voice bank folder against its manifest.

use super::archive::{list_files, sha256_file};
use super::manifest::{parse_manifest, Manifest};
use super::safety::{MANIFEST_NAME, MAX_MANIFEST_BYTES};
use super::wav;
use super::{EvbankError, Result};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Verified {
    pub manifest: Manifest,
    pub file_count: usize,
    pub total_bytes: u64,
    pub warnings: Vec<String>,
}

pub fn file_path(root: &Path, relative: &str) -> PathBuf {
    root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR))
}

pub fn read_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join(MANIFEST_NAME);
    let meta = fs::metadata(&path).map_err(|_| EvbankError::NotABank("manifest.json was not found".into()))?;
    if meta.len() > MAX_MANIFEST_BYTES {
        return Err(EvbankError::Invalid(vec!["manifest.json is larger than 1 MiB".into()]));
    }
    parse_manifest(&fs::read(path)?)
}

/// Full check: manifest rules, exact file set, SHA-256 of every file, WAV headers, model files.
/// This reads every file, so on a large bank it takes a while. It never runs anything from the bank.
pub fn verify_dir(root: &Path) -> Result<Verified> {
    let manifest = read_manifest(root)?;
    let on_disk: BTreeSet<String> = list_files(root)?.into_iter().filter(|f| f != MANIFEST_NAME).collect();
    let declared: BTreeSet<String> = manifest.files.iter().cloned().collect();

    let undeclared: Vec<String> = on_disk.difference(&declared).map(|f| format!("'{f}' is in the bank but not listed in the manifest")).collect();
    if !undeclared.is_empty() {
        return Err(EvbankError::Invalid(undeclared.into_iter().take(20).collect()));
    }

    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    let mut total_bytes = 0u64;
    for file in &manifest.files {
        if !on_disk.contains(file) {
            problems.push(format!("'{file}' is listed in the manifest but missing"));
            continue;
        }
        let path = file_path(root, file);
        match sha256_file(&path) {
            Ok(actual) if actual == manifest.checksums[file] => {}
            Ok(_) => {
                problems.push(format!("'{file}' does not match its checksum (the file was changed or damaged)"));
                continue;
            }
            Err(e) => {
                problems.push(format!("'{file}' cannot be read: {e}"));
                continue;
            }
        }
        let len = fs::metadata(&path)?.len();
        total_bytes += len;
        let lower = file.to_lowercase();
        if lower.ends_with(".wav") {
            let mut handle = File::open(&path)?;
            match wav::read_header(&mut handle, len) {
                Ok(info) => {
                    if file.starts_with("Audio/") && info.sample_rate != manifest.audio_configuration.sample_rate {
                        problems.push(format!(
                            "'{file}' is {} Hz but the manifest says {} Hz",
                            info.sample_rate, manifest.audio_configuration.sample_rate
                        ));
                    }
                }
                Err(why) => problems.push(format!("'{file}' is not a usable WAV: {why}")),
            }
        } else if lower.ends_with(".onnx") && len == 0 {
            problems.push(format!("model '{file}' is empty"));
        }
    }
    if manifest.synthesis_types.iter().any(|t| t == "neural") && !manifest.files.iter().any(|f| f.to_lowercase().ends_with(".onnx")) {
        warnings.push("the bank declares neural synthesis but contains no .onnx model".to_string());
    }
    if manifest.synthesis_types.iter().any(|t| t == "sample") && !manifest.files.iter().any(|f| f.to_lowercase().ends_with(".wav")) {
        warnings.push("the bank declares sample synthesis but contains no .wav audio".to_string());
    }
    if !problems.is_empty() {
        problems.truncate(30);
        return Err(EvbankError::Corrupted(problems));
    }
    Ok(Verified { file_count: manifest.files.len(), manifest, total_bytes, warnings })
}
