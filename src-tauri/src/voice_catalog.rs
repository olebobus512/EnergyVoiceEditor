use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Emitter, Manager, State};
const REVISION: &str = "e9ab671f2a0e50a701b34361136ba3e6a3a9eb0c";
const STEM: &str = "en_US-ljspeech-high";
const MODEL_HASH: &str = "5d4f08ba6a2a48c44592eed3ce56bf85e9de3dd4e20df90541ae68a8310c029a";
const CONFIG_HASH: &str = "7e1f4634af596d83cca997fb7a931ba80b70f8a316a2655ee69c55365e0ace14";
#[derive(Default)]
pub struct DownloadState { busy: Arc<AtomicBool>, cancel: Arc<AtomicBool> }
struct Busy(Arc<AtomicBool>);
impl Drop for Busy { fn drop(&mut self) { self.0.store(false, Ordering::SeqCst); } }
struct Stage(PathBuf);
impl Drop for Stage { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct CatalogVoice { id: &'static str, name: &'static str, language: &'static str, bytes: u64, model_card: String, dataset_license: &'static str, installed: bool }
fn root(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("voices/piper");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?; Ok(path)
}
fn stage(app: &AppHandle) -> Result<Stage, String> {
    // Outside piper/: partially downloaded models must never appear in discovery.
    let path = root(app)?.parent().unwrap().join(format!(".install-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()));
    fs::create_dir(&path).map_err(|e| e.to_string())?; Ok(Stage(path))
}
#[tauri::command]
pub fn voice_catalog(app: AppHandle) -> Result<Vec<CatalogVoice>, String> {
    Ok(vec![CatalogVoice { id: STEM, name: "LJ Speech · High", language: "en-US", bytes: 114199011, model_card: format!("https://huggingface.co/rhasspy/piper-voices/blob/{REVISION}/en/en_US/ljspeech/high/MODEL_CARD"), dataset_license: "Public domain (dataset, according to MODEL_CARD)", installed: root(&app)?.join(STEM).join(format!("{STEM}.onnx")).is_file() }])
}
fn validate_config(path: &Path) -> Result<(), String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 1024 * 1024 { return Err("Piper configuration exceeds 1 MB".into()); }
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.len() > 1024 * 1024 { return Err("Piper configuration exceeds 1 MB".into()); }
    let value: serde_json::Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    let rate = value["audio"]["sample_rate"].as_u64().ok_or("Missing audio.sample_rate")?;
    if !(8000..=192000).contains(&rate) || !value["phoneme_id_map"].is_object() { return Err("Invalid Piper configuration".into()); }
    Ok(())
}
fn publish(stage: &Stage, target: &Path) -> Result<(), String> {
    if target.exists() { return Err("Voice already installed".into()); }
    fs::rename(&stage.0, target).map_err(|e| e.to_string())
}
fn download_file(client: &reqwest::blocking::Client, url: &str, path: &Path, expected: &str, limit: u64, cancel: &AtomicBool, progress: impl Fn(u64)) -> Result<(), String> {
    let mut response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|e| format!("{e:?}"))?;
    let mut file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new(); let mut size = 0u64; let mut buffer = [0u8; 65536];
    loop {
        if cancel.load(Ordering::SeqCst) { return Err("Download cancelled".into()); }
        let n = response.read(&mut buffer).map_err(|e| e.to_string())?; if n == 0 { break; }
        size += n as u64; if size > limit { return Err("Download exceeds size limit".into()); }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?; hash.update(&buffer[..n]); progress(size);
    }
    if format!("{:x}", hash.finalize()) != expected { return Err("Downloaded file SHA-256 does not match the catalogue".into()); }
    Ok(())
}
#[tauri::command]
pub fn cancel_voice_download(state: State<'_, DownloadState>) { state.cancel.store(true, Ordering::SeqCst); }
#[tauri::command]
pub async fn download_voice(app: AppHandle, state: State<'_, DownloadState>, id: String) -> Result<(), String> {
    if id != STEM { return Err("Unknown catalogue voice".into()); }
    if state.busy.swap(true, Ordering::SeqCst) { return Err("A voice installation is already running".into()); }
    let busy = Busy(state.busy.clone()); let cancel = state.cancel.clone(); cancel.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let _busy = busy; let staging = stage(&app)?;
        let target = root(&app)?.join(STEM); if target.exists() { return Err("Voice already installed".into()); }
        let client = reqwest::blocking::Client::builder().https_only(true).connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(600)).build().map_err(|e| e.to_string())?;
        let base = format!("https://huggingface.co/rhasspy/piper-voices/resolve/{REVISION}/en/en_US/ljspeech/high/{STEM}");
        download_file(&client, &format!("{base}.onnx"), &staging.0.join(format!("{STEM}.onnx")), MODEL_HASH, 114199011, &cancel, |bytes| { let _ = app.emit("voice-download-progress", bytes as f64 / 114199011.0); })?;
        let config = staging.0.join(format!("{STEM}.onnx.json"));
        download_file(&client, &format!("{base}.onnx.json"), &config, CONFIG_HASH, 1024*1024, &cancel, |_| {})?;
        validate_config(&config)?;
        if cancel.load(Ordering::SeqCst) { return Err("Download cancelled".into()); }
        publish(&staging, &target)
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn import_piper_voice(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let model = Path::new(&path);
        if model.extension().is_none_or(|v| v != "onnx") { return Err("Select an .onnx model".into()); }
        let name = model.file_stem().and_then(|v| v.to_str()).ok_or("Invalid file name")?;
        if name.is_empty() || name.len() > 100 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("Model name must contain letters, digits, - or _".into()); }
        let bytes = fs::metadata(model).map_err(|e| e.to_string())?.len();
        if !(1024..=512*1024*1024).contains(&bytes) { return Err("Model must be between 1 KB and 512 MB".into()); }
        let config = PathBuf::from(format!("{path}.json")); validate_config(&config)?;
        let staging = stage(&app)?;
        fs::copy(model, staging.0.join(format!("{name}.onnx"))).map_err(|e| e.to_string())?;
        fs::copy(config, staging.0.join(format!("{name}.onnx.json"))).map_err(|e| e.to_string())?;
        publish(&staging, &root(&app)?.join(name))
    }).await.map_err(|e| e.to_string())?
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn configuration_rejects_foreign_json() {
        let path = std::env::temp_dir().join(format!("eve-piper-config-{}.json", std::process::id()));
        fs::write(&path, r#"{"audio":{"sample_rate":22050},"phoneme_id_map":{"a":[1]}}"#).unwrap();
        assert!(validate_config(&path).is_ok());
        fs::write(&path, r#"{"audio":{"sample_rate":0}}"#).unwrap(); assert!(validate_config(&path).is_err());
        let _ = fs::remove_file(path);
    }
    #[test]
    #[ignore = "uses the public Hugging Face server"]
    fn pinned_catalog_config_download_verifies_hash() {
        let path = std::env::temp_dir().join(format!("eve-piper-network-{}.json", std::process::id()));
        let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).build().unwrap();
        let url = format!("https://huggingface.co/rhasspy/piper-voices/resolve/{REVISION}/en/en_US/ljspeech/high/{STEM}.onnx.json");
        let result = download_file(&client, &url, &path, CONFIG_HASH, 1024*1024, &AtomicBool::new(false), |_| {}).and_then(|_| validate_config(&path));
        let _ = fs::remove_file(path);
        result.unwrap();
    }
}
