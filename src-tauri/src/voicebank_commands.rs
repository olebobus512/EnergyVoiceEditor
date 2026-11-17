//! Tauri commands for the Voicebank Manager. All real work lives in `evbank`; these only resolve the store
//! folder, move the work off the UI thread and turn errors into `{ code, message }` for the frontend.

use crate::evbank::archive::pack;
use crate::evbank::store::{BankInfo, InstallOutcome, VoicebankStore};
use crate::evbank::EvbankError;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const MAX_PREVIEW_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Serialize)]
pub struct CommandError {
    /// not_a_bank, invalid, corrupted, unsafe, unsupported_version, already_installed, not_installed, io
    pub code: &'static str,
    pub message: String,
    /// Set for already_installed so the UI can ask before replacing.
    pub installed: Option<String>,
    pub incoming: Option<String>,
}

impl From<EvbankError> for CommandError {
    fn from(error: EvbankError) -> Self {
        let code = match &error {
            EvbankError::NotABank(_) => "not_a_bank",
            EvbankError::Invalid(_) => "invalid",
            EvbankError::Corrupted(_) => "corrupted",
            EvbankError::Unsafe(_) => "unsafe",
            EvbankError::UnsupportedVersion { .. } => "unsupported_version",
            EvbankError::AlreadyInstalled { .. } => "already_installed",
            EvbankError::NotInstalled(_) => "not_installed",
            EvbankError::Io(_) => "io",
        };
        let (installed, incoming) = match &error {
            EvbankError::AlreadyInstalled { installed, incoming, .. } => (Some(installed.clone()), Some(incoming.clone())),
            _ => (None, None),
        };
        CommandError { code, message: error.to_string(), installed, incoming }
    }
}

type Reply<T> = Result<T, CommandError>;

fn io_error(message: String) -> CommandError {
    CommandError { code: "io", message, installed: None, incoming: None }
}

fn store_dir(app: &AppHandle) -> Reply<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("voicebanks"))
        .map_err(|e| io_error(format!("Cannot resolve app data directory: {e}")))
}

fn open_store(app: &AppHandle) -> Reply<VoicebankStore> {
    Ok(VoicebankStore::new(store_dir(app)?)?)
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Reply<T> + Send + 'static) -> Reply<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| io_error(format!("The background task failed: {e}")))?
}

#[tauri::command]
pub async fn evbank_list(app: AppHandle) -> Reply<Vec<BankInfo>> {
    blocking(move || Ok(open_store(&app)?.list())).await
}

/// Installs a `.evbank` archive (`kind` = "archive") or an unpacked folder (`kind` = "folder").
#[tauri::command]
pub async fn evbank_install(app: AppHandle, path: String, kind: String, replace: bool) -> Reply<InstallOutcome> {
    blocking(move || {
        let store = open_store(&app)?;
        let source = PathBuf::from(path);
        let outcome = match kind.as_str() {
            "folder" => store.install_folder(&source, replace),
            _ => store.install_archive(&source, replace),
        };
        Ok(outcome?)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifySummary {
    pub file_count: usize,
    pub total_bytes: u64,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub async fn evbank_verify(app: AppHandle, id: String) -> Reply<VerifySummary> {
    blocking(move || {
        let verified = open_store(&app)?.verify(&id)?;
        Ok(VerifySummary { file_count: verified.file_count, total_bytes: verified.total_bytes, warnings: verified.warnings })
    })
    .await
}

#[tauri::command]
pub async fn evbank_uninstall(app: AppHandle, id: String) -> Reply<()> {
    blocking(move || Ok(open_store(&app)?.uninstall(&id)?)).await
}

#[tauri::command]
pub async fn evbank_export(app: AppHandle, id: String, destination: String) -> Reply<()> {
    blocking(move || Ok(open_store(&app)?.export(&id, &PathBuf::from(destination))?)).await
}

/// The authoring tool: packs a folder with a hand-written `manifest.json` into a `.evbank`, computing `files` and `checksums`.
#[tauri::command]
pub async fn evbank_pack(folder: String, destination: String) -> Reply<String> {
    blocking(move || {
        let manifest = pack(&PathBuf::from(folder), &PathBuf::from(destination))?;
        Ok(manifest.voice_id)
    })
    .await
}

#[tauri::command]
pub fn evbank_dir(app: AppHandle) -> Reply<String> {
    let store = open_store(&app)?;
    Ok(store.root().to_string_lossy().into_owned())
}

/// The bank's preview audio (a WAV named in the manifest), for the Preview button.
#[tauri::command]
pub async fn evbank_preview(app: AppHandle, id: String) -> Result<tauri::ipc::Response, CommandError> {
    let bytes = blocking(move || {
        let store = open_store(&app)?;
        let path = store
            .preview_path(&id)?
            .ok_or_else(|| io_error("This voice bank has no preview.".to_string()))?;
        let size = std::fs::metadata(&path).map_err(|e| io_error(e.to_string()))?.len();
        if size > MAX_PREVIEW_BYTES {
            return Err(io_error("The preview file is too large.".to_string()));
        }
        std::fs::read(&path).map_err(|e| io_error(format!("Cannot read the preview: {e}")))
    })
    .await?;
    Ok(tauri::ipc::Response::new(bytes))
}
