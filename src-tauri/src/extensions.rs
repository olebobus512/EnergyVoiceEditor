use crate::audio::Audio;
use serde::{Deserialize, Serialize};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instrument {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub harmonics: Vec<f64>,
    pub attack: f64,
    pub release: f64,
    pub decay: f64,
}
impl Instrument {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.id.is_empty() || self.id.len() > 64 || !self.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') { return Err("Invalid instrument id or version".into()); }
        if self.name.trim().is_empty() || self.name.len() > 120 || self.harmonics.is_empty() || self.harmonics.len() > 32 || self.harmonics.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) || self.harmonics.iter().sum::<f64>() <= 0.0 { return Err("Invalid instrument name or harmonics".into()); }
        if !(0.001..=2.0).contains(&self.attack) || !(0.001..=3.0).contains(&self.release) || !(0.0..=20.0).contains(&self.decay) { return Err("Invalid envelope".into()); }
        Ok(())
    }
    pub fn render(&self, midi: u8, duration: f64, rate: u32) -> Audio {
        let frequency = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
        let norm = self.harmonics.iter().sum::<f64>().max(1.0);
        let samples = (0..(duration * rate as f64).ceil() as usize).map(|i| {
            let t = i as f64 / rate as f64;
            let envelope = (t / self.attack).min(1.0) * ((duration - t) / self.release).clamp(0.0, 1.0) * (-t * self.decay).exp();
            let signal: f64 = self.harmonics.iter().enumerate().filter(|(h, _)| frequency * ((*h + 1) as f64) < rate as f64 * 0.45).map(|(h, weight)| (std::f64::consts::TAU * frequency * (h + 1) as f64 * t).sin() * weight).sum();
            (signal / norm * envelope * 0.4) as f32
        }).collect();
        Audio { sample_rate: rate, channels: 1, samples }
    }
}
fn root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?.join("instruments");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}
fn read_instrument(path: &Path) -> Result<Instrument, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 16384 { return Err("Instrument manifest exceeds 16 KB".into()); }
    let data: Instrument = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    data.validate()?;
    Ok(data)
}
#[tauri::command]
pub fn list_instruments(app: AppHandle) -> Result<Vec<Instrument>, String> {
    let mut items = Vec::new();
    for item in fs::read_dir(root(&app)?).map_err(|e| e.to_string())? {
        let path = item.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|v| v == "json") { items.push(read_instrument(&path)?); }
    }
    items.sort_by(|a,b| a.name.cmp(&b.name));
    Ok(items)
}
#[tauri::command]
pub fn install_instrument(app: AppHandle, path: String) -> Result<Instrument, String> {
    let instrument = read_instrument(Path::new(&path))?;
    let folder = root(&app)?;
    let target = folder.join(format!("{}.json", instrument.id));
    let staging = folder.join(format!(".{}.install", instrument.id));
    // The staging file also serializes concurrent installation of the same id.
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&staging).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        if target.exists() { return Err("Instrument already installed".into()); }
        file.write_all(&serde_json::to_vec_pretty(&instrument).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&staging, &target).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_file(staging);
    result?;
    Ok(instrument)
}
pub fn resolve(app: &AppHandle, id: &str) -> Result<Option<Instrument>, String> {
    let Some(id) = id.strip_prefix("plugin:") else { return Ok(None) };
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') { return Err("Invalid plugin id".into()); }
    Ok(Some(read_instrument(&root(app)?.join(format!("{id}.json")))?))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetNote { pub midi: u8, pub start: f64, pub duration: f64, #[serde(default)] pub lyric: String }
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset { pub name: String, pub notes: Vec<PresetNote>, pub tempo: Option<f64> }
fn validate_preset(preset: &Preset) -> Result<(), String> {
    if preset.name.trim().is_empty() || preset.name.len() > 120 || preset.notes.is_empty() || preset.notes.len() > 2000 { return Err("Invalid preset name or note count".into()); }
    if preset.tempo.is_some_and(|v| !(30.0..=300.0).contains(&v)) { return Err("Tempo must be 30–300".into()); }
    for note in &preset.notes {
        if note.midi > 127 || !(0.0..=4096.0).contains(&note.start) || !(0.0625..=64.0).contains(&note.duration) || note.lyric.chars().count() > 100 { return Err("Invalid preset note".into()); }
    }
    Ok(())
}
fn run_script(path: &Path, input: &str) -> Result<Preset, String> {
    if path.extension().is_none_or(|e| e != "py") || !path.is_file() { return Err("Select a Python .py file".into()); }
    if input.len() > 1024 * 1024 { return Err("Script input exceeds 1 MB".into()); }
    let mut command = Command::new(std::env::var("ENERGY_PYTHON").unwrap_or_else(|_| "python".into()));
    command.arg("-X").arg("utf8").arg(path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("Missing stdin")?;
    let input = input.as_bytes().to_vec();
    std::thread::spawn(move || { let _ = stdin.write_all(&input); });
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let (tx, rx) = std::sync::mpsc::channel();
    for (kind, reader) in [(0, Box::new(stdout) as Box<dyn Read + Send>), (1, Box::new(stderr) as Box<dyn Read + Send>)] {
        let tx = tx.clone();
        std::thread::spawn(move || { let mut bytes = Vec::new(); let result = reader.take(1024 * 1024 + 1).read_to_end(&mut bytes); let _ = tx.send((kind, result.map(|_| bytes))); });
    }
    drop(tx);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut output = None;
    let mut errors = None;
    loop {
        while let Ok((kind, data)) = rx.try_recv() {
            let bytes = match data { Ok(bytes) => bytes, Err(e) => { let _ = child.kill(); let _ = child.wait(); return Err(e.to_string()); } };
            if bytes.len() > 1024 * 1024 { let _ = child.kill(); let _ = child.wait(); return Err("Python output exceeds 1 MB".into()); }
            if kind == 0 { output = Some(bytes); } else { errors = Some(bytes); }
        }
        if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); return Err("Python preset timed out after 30 seconds".into()); }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if let (Some(out), Some(err)) = (&output, &errors) {
                if !status.success() { return Err(String::from_utf8_lossy(err).chars().take(4000).collect()); }
                let preset: Preset = serde_json::from_slice(out).map_err(|e| format!("Python must return preset JSON: {e}"))?;
                validate_preset(&preset)?;
                return Ok(preset);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[tauri::command]
pub async fn run_python_preset(path: String, input: String) -> Result<Preset, String> {
    tauri::async_runtime::spawn_blocking(move || run_script(Path::new(&path), &input)).await.map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn instrument_validation_and_render() {
        let mut instrument = Instrument { id: "bell".into(), name: "Bell".into(), version: 1, harmonics: vec![1.0, 0.3], attack: 0.01, release: 0.1, decay: 2.0 };
        assert!(instrument.validate().is_ok());
        let audio = instrument.render(60, 0.2, 22050);
        assert_eq!(audio.samples.len(), 4410);
        assert!(audio.samples.iter().all(|v| v.is_finite() && v.abs() <= 0.4));
        instrument.id = "../escape".into(); assert!(instrument.validate().is_err());
        instrument.id = "bell".into(); instrument.attack = f64::NAN; assert!(instrument.validate().is_err());
    }
    #[test] fn invalid_preset_is_rejected() {
        let mut preset = Preset { name: "Test".into(), tempo: Some(120.0), notes: vec![PresetNote { midi: 60, start: 0.0, duration: 1.0, lyric: "la".into() }] };
        assert!(validate_preset(&preset).is_ok());
        preset.notes[0].start = f64::NAN; assert!(validate_preset(&preset).is_err());
    }
    #[test] fn example_python_preset_runs_through_protocol() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/extensions/arpeggio.py");
        let preset = run_script(&path, r#"{"apiVersion":1,"project":{},"selectedTrackId":""}"#).unwrap();
        assert_eq!(preset.notes.len(), 32);
        assert_eq!(preset.notes[0].midi, 60);
        assert_eq!(preset.tempo, Some(120.0));
    }
}

