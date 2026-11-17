use crate::audio;
use crate::settings::{self, Settings};
use crate::worker::PythonWorker;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

const WORKER_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../engines/python-runtime/main.py");
/// Engines the worker should load. It skips the ones that are unavailable on this machine and only offers the
/// debug tone generator when no real speech engine has a voice.
const ENGINE_NAMES: [&str; 3] = ["sapi", "piper", "debug"];
const MAX_TEXT_LENGTH: usize = 2000;

pub type CmdResult<T> = Result<T, String>;

/// Shared handle to the single Python worker. `None` until the first synthesis-related call.
#[derive(Default)]
pub struct WorkerState(Arc<Mutex<Option<PythonWorker>>>);

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub languages: Vec<String>,
    /// Engine that provides the voice ("sapi", "piper", "debug").
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub gender: String,
    #[serde(default = "default_true")]
    pub supports_pitch: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisResult {
    pub path: String,
    pub duration_sec: f64,
    pub sample_rate: u32,
}

fn ensure_worker<'a>(
    slot: &'a mut Option<PythonWorker>,
    app: &AppHandle,
) -> CmdResult<&'a mut PythonWorker> {
    if slot.is_none() {
        let log_app = app.clone();
        let script = if cfg!(debug_assertions) {
            PathBuf::from(WORKER_SCRIPT)
        } else {
            app.path().resource_dir().map_err(|e| e.to_string())?.join("python-runtime/main.py")
        };
        let mut worker = PythonWorker::spawn(python_executable().as_str(), &script, move |line| {
            let _ = log_app.emit("worker-log", line);
        })?;
        let voices_dir = voices_root(app)?;
        worker.request(
            "init",
            json!({ "engines": ENGINE_NAMES, "voices_dir": voices_dir.to_string_lossy() }),
            |_| {},
        )?;
        *slot = Some(worker);
    }
    Ok(slot.as_mut().expect("worker was just initialized"))
}

/// Folder for user-installed voice models (`voices/piper/` inside it holds Piper models). Created on demand.
fn voices_root(app: &AppHandle) -> CmdResult<PathBuf> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {e}"))?
        .join("voices");
    let piper = root.join("piper");
    std::fs::create_dir_all(&piper).map_err(|e| format!("Cannot create {}: {e}", piper.display()))?;
    Ok(root)
}

#[tauri::command]
pub fn get_voices_dir(app: AppHandle) -> CmdResult<String> {
    Ok(voices_root(&app)?.join("piper").to_string_lossy().into_owned())
}

fn python_executable() -> String {
    std::env::var("ENERGY_PYTHON").unwrap_or_else(|_| "python".to_string())
}

fn generated_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {e}"))?
        .join("generated");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn validate_synthesis_params(text: &str, speed: f64, pitch: f64) -> CmdResult<()> {
    if text.trim().is_empty() {
        return Err("Text is empty".into());
    }
    if text.chars().count() > MAX_TEXT_LENGTH {
        return Err(format!("Text is longer than {MAX_TEXT_LENGTH} characters"));
    }
    if !(0.5..=2.0).contains(&speed) {
        return Err("Speed must be between 0.5 and 2.0".into());
    }
    if !(-12.0..=12.0).contains(&pitch) {
        return Err("Pitch must be between -12 and 12 semitones".into());
    }
    Ok(())
}

/// Only files inside the generated-audio directory with a .wav extension may be read.
fn validate_audio_path(dir: &Path, requested: &str) -> CmdResult<PathBuf> {
    let path = Path::new(requested)
        .canonicalize()
        .map_err(|e| format!("Audio file not found: {e}"))?;
    let root = dir.canonicalize().map_err(|e| e.to_string())?;
    let is_wav = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("wav"));
    if !path.starts_with(&root) || !is_wav {
        return Err("Audio path is outside the generated audio directory".into());
    }
    Ok(path)
}

fn now_millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[tauri::command]
pub async fn list_voices(
    app: AppHandle,
    state: State<'_, WorkerState>,
    refresh: Option<bool>,
) -> CmdResult<Vec<Voice>> {
    let refresh = refresh.unwrap_or(false);
    let slot = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<Vec<Voice>> {
        let mut guard = slot.lock().map_err(|_| "Worker lock poisoned".to_string())?;
        let worker = ensure_worker(&mut guard, &app)?;
        let result = worker.request("list_voices", json!({ "refresh": refresh }), |_| {})?;
        serde_json::from_value(result["voices"].clone()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn synthesize(
    app: AppHandle,
    state: State<'_, WorkerState>,
    text: String,
    voice: String,
    speed: f64,
    pitch: f64,
) -> CmdResult<SynthesisResult> {
    validate_synthesis_params(&text, speed, pitch)?;
    let output = generated_dir(&app)?.join(format!("take-{}.wav", now_millis()));
    let slot = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<SynthesisResult> {
        let mut guard = slot.lock().map_err(|_| "Worker lock poisoned".to_string())?;
        let worker = ensure_worker(&mut guard, &app)?;
        let progress_app = app.clone();
        let params = json!({
            "text": text,
            "voice": voice,
            "speed": speed,
            "pitch": pitch,
            "output_path": output.to_string_lossy(),
        });
        let result = worker.request("synthesize", params, move |event: &Value| {
            let _ = progress_app.emit("synthesis-progress", event["progress"].clone());
        })?;
        Ok(SynthesisResult {
            path: result["path"].as_str().unwrap_or_default().to_string(),
            duration_sec: result["duration_sec"].as_f64().unwrap_or_default(),
            sample_rate: result["sample_rate"].as_u64().unwrap_or_default() as u32,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn preview_note(app: AppHandle, instrument: String, midi: u8, lyric: String) -> CmdResult<Vec<f32>> {
    if midi > 127 || lyric.chars().count() > 100 { return Err("Invalid preview note".into()); }
    let audio = match crate::extensions::resolve(&app, &instrument)? {
        Some(plugin) => plugin.render(midi, 0.5, 22050),
        None => audio::instrument_note(&instrument, midi, &lyric, 0.5, 22050),
    };
    Ok(audio.samples)
}

#[tauri::command]
pub fn read_audio(app: AppHandle, path: String) -> CmdResult<tauri::ipc::Response> {
    let dir = generated_dir(&app)?;
    let file = validate_audio_path(&dir, &path)?;
    let bytes = std::fs::read(&file).map_err(|e| format!("Cannot read audio: {e}"))?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub fn export_audio(app: AppHandle, path: String) -> CmdResult<String> {
    let source = validate_audio_path(&generated_dir(&app)?, &path)?;
    let exports = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {e}"))?
        .join("exports");
    std::fs::create_dir_all(&exports).map_err(|e| format!("Cannot create {}: {e}", exports.display()))?;
    let target = exports.join(format!("export-{}.wav", now_millis()));
    std::fs::copy(&source, &target).map_err(|e| format!("Export failed: {e}"))?;
    Ok(target.to_string_lossy().into_owned())
}

fn settings_path(app: &AppHandle) -> CmdResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Cannot resolve config directory: {e}"))?;
    Ok(dir.join("settings.json"))
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> CmdResult<Settings> {
    settings::load_from(&settings_path(&app)?)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<Settings> {
    settings::save_to(&settings_path(&app)?, settings)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub path: String,
    pub duration_sec: f64,
    pub sample_rate: u32,
    pub channels: u16,
}

#[tauri::command]
pub async fn import_audio(path: String) -> CmdResult<AudioInfo> {
    if !path.to_lowercase().ends_with(".wav") {
        return Err("Only WAV files can be imported for now".into());
    }
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<AudioInfo> {
        let audio = audio::read_wav(Path::new(&path))?;
        Ok(AudioInfo {
            duration_sec: audio.duration_sec(),
            sample_rate: audio.sample_rate,
            channels: audio.channels,
            path,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderClip {
    pub path: String,
    pub start: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTrack {
    #[serde(default)]
    pub instrument: String,
    pub volume: f32,
    pub pan: f32,
    pub clips: Vec<RenderClip>,
    #[serde(default)]
    pub notes: Vec<RenderNote>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNote {
    pub midi: u8,
    pub lyric: String,
    pub start: f64,
    pub duration: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub from_sec: f64,
    pub tracks: Vec<RenderTrack>,
    /// Target WAV file for exports. `None` renders a preview into the generated-audio folder.
    pub output: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    pub path: String,
    pub duration_sec: f64,
}

const MAX_TRACKS: usize = 64;
const MAX_CLIPS: usize = 2000;

fn validate_render(request: &RenderRequest) -> CmdResult<()> {
    if !(8000..=192_000).contains(&request.sample_rate) {
        return Err("Sample rate must be between 8000 and 192000 Hz".into());
    }
    if request.channels != 1 && request.channels != 2 {
        return Err("Channels must be 1 (mono) or 2 (stereo)".into());
    }
    if request.bit_depth != 16 && request.bit_depth != 24 {
        return Err("Bit depth must be 16 or 24".into());
    }
    if !request.from_sec.is_finite() || request.from_sec < 0.0 {
        return Err("Render start must be a non-negative time".into());
    }
    if request.tracks.len() > MAX_TRACKS {
        return Err(format!("Too many tracks (max {MAX_TRACKS})"));
    }
    let clip_count: usize = request.tracks.iter().map(|track| track.clips.len()).sum();
    if clip_count > MAX_CLIPS {
        return Err(format!("Too many clips (max {MAX_CLIPS})"));
    }
    let note_count: usize = request.tracks.iter().map(|track| track.notes.len()).sum();
    let note_seconds: f64 = request.tracks.iter().flat_map(|track| &track.notes).map(|note| note.duration).sum();
    if note_count > MAX_CLIPS || !note_seconds.is_finite() || note_seconds > 600.0 {
        return Err("Too many notes or total note duration exceeds 600 seconds".into());
    }
    for track in &request.tracks {
        if !(0.0..=2.0).contains(&track.volume) || !(-1.0..=1.0).contains(&track.pan) {
            return Err("Track volume must be 0..2 and pan -1..1".into());
        }
        for clip in &track.clips {
            if !clip.start.is_finite() || clip.start < 0.0 {
                return Err("Clip start must be a non-negative time".into());
            }
            if !clip.path.to_lowercase().ends_with(".wav") {
                return Err(format!("Clip is not a WAV file: {}", clip.path));
            }
        }
        if track.notes.len() > MAX_CLIPS { return Err("Too many notes".into()); }
        for note in &track.notes {
            if note.midi > 127 || !note.start.is_finite() || !(0.0..=3600.0).contains(&note.start)
                || !note.duration.is_finite() || !(0.01..=60.0).contains(&note.duration)
                || note.start + note.duration > 3600.0 || note.lyric.chars().count() > 100 {
                return Err("Invalid note timing, pitch or lyric".into());
            }
        }
    }
    if let Some(output) = &request.output {
        if !output.to_lowercase().ends_with(".wav") {
            return Err("Exports must be WAV files".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn render_mix(app: AppHandle, request: RenderRequest) -> CmdResult<RenderResult> {
    validate_render(&request)?;
    let output = match &request.output {
        Some(path) => PathBuf::from(path),
        None => {
            let dir = generated_dir(&app)?.join("renders");
            std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
            dir.join(format!("render-{}.wav", now_millis()))
        }
    };
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<RenderResult> {
        let mut tracks = Vec::with_capacity(request.tracks.len());
        for track in &request.tracks {
            let plugin = crate::extensions::resolve(&app, &track.instrument)?;
            let mut clips = Vec::with_capacity(track.clips.len());
            for clip in &track.clips {
                clips.push(audio::MixClip {
                    audio: audio::read_wav(Path::new(&clip.path))?,
                    start: clip.start,
                });
            }
            for note in &track.notes {
                if track.instrument != "vocal" || !note.lyric.trim().is_empty() {
                    clips.push(audio::MixClip {
                        audio: match &plugin {
                            Some(plugin) => plugin.render(note.midi, note.duration, request.sample_rate),
                            None => audio::instrument_note(&track.instrument, note.midi, &note.lyric, note.duration, request.sample_rate),
                        },
                        start: note.start,
                    });
                }
            }
            tracks.push(audio::MixTrack { volume: track.volume, pan: track.pan, clips });
        }
        let mixed = audio::mix(&tracks, request.sample_rate, request.channels, request.from_sec);
        audio::write_wav(&output, &mixed, request.bit_depth)?;
        Ok(RenderResult {
            path: output.to_string_lossy().into_owned(),
            duration_sec: mixed.duration_sec(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub engine_running: bool,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle, state: State<'_, WorkerState>) -> CmdResult<AppInfo> {
    let engine_running = state.0.lock().map(|guard| guard.is_some()).unwrap_or(false);
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        engine_running,
    })
}

/// Stops the Python worker. Waits briefly for a running synthesis to release the lock, then gives up
/// instead of blocking: the app must always be able to exit.
pub fn shutdown_worker(state: &WorkerState) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match state.0.try_lock() {
            Ok(mut guard) => {
                if let Some(worker) = guard.as_mut() {
                    worker.shutdown();
                }
                *guard = None;
                return;
            }
            Err(TryLockError::Poisoned(poisoned)) => {
                *poisoned.into_inner() = None;
                return;
            }
            Err(TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Quits the application. Closing the window is not enough on its own: the window API needs extra permissions,
/// while this goes through the normal exit path, which stops the Python worker and the console shell.
#[tauri::command]
pub fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_synthesis_params() {
        assert!(validate_synthesis_params("", 1.0, 0.0).is_err());
        assert!(validate_synthesis_params("hi", 3.0, 0.0).is_err());
        assert!(validate_synthesis_params("hi", 1.0, 20.0).is_err());
        assert!(validate_synthesis_params(&"a".repeat(2001), 1.0, 0.0).is_err());
        assert!(validate_synthesis_params("hi", 1.0, 0.0).is_ok());
    }

    #[test]
    fn audio_path_must_be_wav_inside_generated_dir() {
        let base = std::env::temp_dir().join(format!("eve-test-{}", now_millis()));
        let inside = base.join("generated");
        std::fs::create_dir_all(&inside).unwrap();
        let wav = inside.join("take.wav");
        let txt = inside.join("notes.txt");
        std::fs::write(&wav, b"RIFF").unwrap();
        std::fs::write(&txt, b"x").unwrap();
        let outside = base.join("secret.wav");
        std::fs::write(&outside, b"RIFF").unwrap();

        assert!(validate_audio_path(&inside, wav.to_str().unwrap()).is_ok());
        assert!(validate_audio_path(&inside, txt.to_str().unwrap()).is_err());
        assert!(validate_audio_path(&inside, outside.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
