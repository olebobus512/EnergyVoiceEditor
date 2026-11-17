//! The installed-bank store: `<app data>/voicebanks/<voiceId>/`.
//!
//! Install is atomic. A bank is unpacked into `.staging/`, fully verified there, and only then renamed into
//! place. An older bank with the same id is moved to `.trash/` first and put back if anything goes wrong.
//! A failed install therefore never leaves a half-installed bank behind.

use super::archive::{copy_folder, export_dir, extract_archive, unique_temp_dir};
use super::manifest::{is_valid_voice_id, version_at_least, License, Manifest};
use super::safety::Limits;
use super::verify::{read_manifest, verify_dir, Verified};
use super::{EvbankError, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const STAGING: &str = ".staging";
const TRASH: &str = ".trash";

/// Deletes a folder when dropped, unless `keep()` was called.
struct TempDir {
    path: PathBuf,
    keep: bool,
}

impl TempDir {
    fn new(parent: &Path, label: &str) -> Result<Self> {
        Ok(Self { path: unique_temp_dir(parent, label)?, keep: false })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The package is valid and its dependencies are installed.
    Ready,
    MissingDependencies,
    /// Written for a newer format version than this app understands.
    Incompatible,
    Corrupted,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BankInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub languages: Vec<String>,
    pub synthesis_types: Vec<String>,
    pub supported_backends: Vec<String>,
    /// Backends from `supportedBackends` that exist in this build. Empty means no synthesis can use the bank yet.
    pub usable_backends: Vec<String>,
    pub license: Option<License>,
    pub tags: Vec<String>,
    pub status: Status,
    pub problems: Vec<String>,
    pub size_bytes: u64,
    pub path: String,
    pub has_preview: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    pub id: String,
    pub name: String,
    pub version: String,
    /// Version that was replaced, if the bank was installed before.
    pub replaced: Option<String>,
    pub warnings: Vec<String>,
}

pub struct VoicebankStore {
    root: PathBuf,
    limits: Limits,
}

impl VoicebankStore {
    pub fn new(root: PathBuf) -> Result<Self> {
        Self::with_limits(root, Limits::default())
    }

    pub fn with_limits(root: PathBuf, limits: Limits) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let store = Self { root, limits };
        store.clean_leftovers();
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Removes staging and trash folders left behind by a crash during an earlier install.
    fn clean_leftovers(&self) {
        for name in [STAGING, TRASH] {
            let _ = fs::remove_dir_all(self.root.join(name));
        }
    }

    fn bank_dir(&self, id: &str) -> Result<PathBuf> {
        if !is_valid_voice_id(id) {
            return Err(EvbankError::Invalid(vec![format!("'{id}' is not a valid voice id")]));
        }
        Ok(self.root.join(id))
    }

    pub fn install_archive(&self, archive: &Path, replace: bool) -> Result<InstallOutcome> {
        let staging = self.staging()?;
        extract_archive(archive, &staging.path, &self.limits)?;
        self.install_staged(staging, replace)
    }

    pub fn install_folder(&self, folder: &Path, replace: bool) -> Result<InstallOutcome> {
        let staging = self.staging()?;
        copy_folder(folder, &staging.path, &self.limits)?;
        self.install_staged(staging, replace)
    }

    fn staging(&self) -> Result<TempDir> {
        let parent = self.root.join(STAGING);
        fs::create_dir_all(&parent)?;
        TempDir::new(&parent, "bank")
    }

    fn install_staged(&self, mut staging: TempDir, replace: bool) -> Result<InstallOutcome> {
        let verified = verify_dir(&staging.path)?;
        let manifest = &verified.manifest;
        let target = self.bank_dir(&manifest.voice_id)?;
        let mut replaced = None;
        let mut trashed: Option<TempDir> = None;
        if target.exists() {
            let installed = read_manifest(&target).map(|m| m.version_text()).unwrap_or_else(|_| "damaged".to_string());
            if !replace {
                return Err(EvbankError::AlreadyInstalled { id: manifest.voice_id.clone(), installed, incoming: manifest.version_text() });
            }
            let trash = TempDir::new(&self.root.join(TRASH).tap_create()?, "old")?;
            fs::rename(&target, trash.path.join("bank")).map_err(|e| EvbankError::Io(format!("cannot replace the installed bank: {e}")))?;
            replaced = Some(installed);
            trashed = Some(trash);
        }
        if let Err(error) = fs::rename(&staging.path, &target) {
            if let Some(trash) = &trashed {
                let _ = fs::rename(trash.path.join("bank"), &target);
            }
            return Err(EvbankError::Io(format!("cannot move the bank into place: {error}")));
        }
        staging.keep = true;
        drop(trashed);
        Ok(InstallOutcome {
            id: manifest.voice_id.clone(),
            name: manifest.voice_name.clone(),
            version: manifest.version_text(),
            replaced,
            warnings: verified.warnings,
        })
    }

    /// Reads every installed bank's manifest and gives it a status. This is quick: it does not re-hash files
    /// (use `verify` for that), but it notices missing files.
    pub fn list(&self) -> Vec<BankInfo> {
        let Ok(entries) = fs::read_dir(&self.root) else { return Vec::new() };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect();
        names.sort();
        let manifests: Vec<(String, Result<Manifest>)> = names.iter().map(|n| (n.clone(), read_manifest(&self.root.join(n)))).collect();
        let installed: Vec<(&str, String)> =
            manifests.iter().filter_map(|(n, m)| m.as_ref().ok().map(|m| (n.as_str(), m.version_text()))).collect();
        manifests.iter().map(|(name, manifest)| self.describe(name, manifest, &installed)).collect()
    }

    fn describe(&self, dir_name: &str, manifest: &Result<Manifest>, installed: &[(&str, String)]) -> BankInfo {
        let path = self.root.join(dir_name);
        let broken = |status: Status, why: String| BankInfo {
            id: dir_name.to_string(),
            name: dir_name.to_string(),
            version: String::new(),
            author: String::new(),
            description: String::new(),
            languages: vec![],
            synthesis_types: vec![],
            supported_backends: vec![],
            usable_backends: vec![],
            license: None,
            tags: vec![],
            status,
            problems: vec![why],
            size_bytes: 0,
            path: path.to_string_lossy().into_owned(),
            has_preview: false,
        };
        let manifest = match manifest {
            Ok(m) => m,
            Err(EvbankError::UnsupportedVersion { .. }) => {
                return broken(Status::Incompatible, manifest.as_ref().err().map(|e| e.to_string()).unwrap_or_default());
            }
            Err(error) => return broken(Status::Corrupted, error.to_string()),
        };
        let mut problems = Vec::new();
        let mut size = 0u64;
        for file in &manifest.files {
            match fs::metadata(super::verify::file_path(&path, file)) {
                Ok(meta) => size += meta.len(),
                Err(_) => problems.push(format!("'{file}' is missing")),
            }
        }
        let mut status = if problems.is_empty() { Status::Ready } else { Status::Corrupted };
        if status == Status::Ready {
            for dep in &manifest.dependencies {
                let have = installed.iter().find(|(id, _)| *id == dep.voice_id).map(|(_, version)| version.as_str());
                match (have, &dep.min_version) {
                    (None, _) => problems.push(format!("requires voice bank '{}', which is not installed", dep.voice_id)),
                    (Some(have), Some(need)) if !version_at_least(have, need) => {
                        problems.push(format!("requires '{}' {need} or newer (installed: {have})", dep.voice_id))
                    }
                    _ => {}
                }
            }
            if !problems.is_empty() {
                status = Status::MissingDependencies;
            }
        }
        BankInfo {
            id: manifest.voice_id.clone(),
            name: manifest.voice_name.clone(),
            version: manifest.version_text(),
            author: manifest.author.clone(),
            description: manifest.description.clone(),
            languages: manifest.languages.clone(),
            synthesis_types: manifest.synthesis_types.clone(),
            supported_backends: manifest.supported_backends.clone(),
            usable_backends: manifest.usable_backends(),
            license: Some(manifest.license.clone()),
            tags: manifest.tags.clone(),
            status,
            problems,
            size_bytes: size,
            path: path.to_string_lossy().into_owned(),
            has_preview: manifest.preview.is_some(),
        }
    }

    pub fn verify(&self, id: &str) -> Result<Verified> {
        let dir = self.bank_dir(id)?;
        if !dir.is_dir() {
            return Err(EvbankError::NotInstalled(id.to_string()));
        }
        verify_dir(&dir)
    }

    pub fn uninstall(&self, id: &str) -> Result<()> {
        let dir = self.bank_dir(id)?;
        if !dir.is_dir() {
            return Err(EvbankError::NotInstalled(id.to_string()));
        }
        // Move out of the way first so a half-deleted folder is never mistaken for an installed bank.
        let trash = TempDir::new(&self.root.join(TRASH).tap_create()?, "removed")?;
        fs::rename(&dir, trash.path.join("bank")).map_err(|e| EvbankError::Io(format!("cannot remove the bank (is a file in use?): {e}")))?;
        Ok(())
    }

    /// Writes an installed bank to a `.evbank` file after verifying it.
    pub fn export(&self, id: &str, destination: &Path) -> Result<()> {
        self.verify(id)?;
        export_dir(&self.bank_dir(id)?, destination)
    }

    /// Absolute path of the bank's preview file, if the manifest names one.
    pub fn preview_path(&self, id: &str) -> Result<Option<PathBuf>> {
        let dir = self.bank_dir(id)?;
        let manifest = read_manifest(&dir).map_err(|_| EvbankError::NotInstalled(id.to_string()))?;
        Ok(manifest.preview.map(|p| super::verify::file_path(&dir, &p)).filter(|p| p.is_file()))
    }
}

trait CreateDir: Sized {
    fn tap_create(self) -> Result<Self>;
}

impl CreateDir for PathBuf {
    fn tap_create(self) -> Result<Self> {
        fs::create_dir_all(&self)?;
        Ok(self)
    }
}
