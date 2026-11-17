//! The `manifest.json` of a voice bank: types, and validation that reports every problem in readable words.

use super::safety::{check_relative_path, MANIFEST_NAME};
use super::{EvbankError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT_NAME: &str = "evbank";
/// Highest format version this build understands.
pub const FORMAT_VERSION: u32 = 1;
pub const SYNTHESIS_TYPES: [&str; 4] = ["sample", "neural", "speech", "singing"];
/// Synthesis backends that exist in this build and can actually use a bank. None yet (see IMPLEMENTATION_ROADMAP.md).
pub const IMPLEMENTED_BACKENDS: &[&str] = &[];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfiguration {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PitchRange {
    pub min_midi: u8,
    pub max_midi: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub commercial_use: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub voice_id: String,
    #[serde(default)]
    pub min_version: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceVariant {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub voice_id: String,
    pub voice_name: String,
    #[serde(default)]
    pub version: Option<String>,
    pub author: String,
    pub description: String,
    pub languages: Vec<String>,
    pub synthesis_types: Vec<String>,
    pub supported_backends: Vec<String>,
    pub phonemizers: Vec<String>,
    pub audio_configuration: AudioConfiguration,
    pub pitch_range: PitchRange,
    pub license: License,
    pub files: Vec<String>,
    pub checksums: BTreeMap<String, String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub supported_expressions: Vec<String>,
    #[serde(default)]
    pub model_requirements: Option<Value>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub voice_variants: Vec<VoiceVariant>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Manifest {
    /// The bank's own version, or "0.0.0" when the manifest does not say.
    pub fn version_text(&self) -> String {
        self.version.clone().unwrap_or_else(|| "0.0.0".to_string())
    }

    /// Backends listed in the manifest that this build can run.
    pub fn usable_backends(&self) -> Vec<String> {
        self.supported_backends
            .iter()
            .filter(|backend| IMPLEMENTED_BACKENDS.contains(&backend.as_str()))
            .cloned()
            .collect()
    }
}

pub fn is_valid_voice_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (3..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
        && !id.contains("..")
}

fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let first = parts.next().unwrap_or("");
    (2..=3).contains(&first.len())
        && first.chars().all(|c| c.is_ascii_alphabetic())
        && parts.all(|part| (2..=8).contains(&part.len()) && part.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Dotted numbers such as "1.2.3"; anything non-numeric after a dot or dash is ignored. Used to compare versions.
pub fn parse_version(text: &str) -> Vec<u64> {
    text.split(|c| c == '-' || c == '+')
        .next()
        .unwrap_or("")
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn version_at_least(have: &str, need: &str) -> bool {
    let (mut a, mut b) = (parse_version(have), parse_version(need));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a >= b
}

fn text<'a>(value: &'a Value, key: &str, problems: &mut Vec<String>) -> Option<&'a str> {
    match value.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s),
        Some(Value::String(_)) => {
            problems.push(format!("'{key}' must not be empty"));
            None
        }
        Some(_) => {
            problems.push(format!("'{key}' must be text"));
            None
        }
        None => {
            problems.push(format!("required field '{key}' is missing"));
            None
        }
    }
}

fn string_list(value: &Value, key: &str, required_non_empty: bool, problems: &mut Vec<String>) -> Vec<String> {
    match value.get(key) {
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(s) if !s.trim().is_empty() => out.push(s.to_string()),
                    _ => problems.push(format!("'{key}' must contain only non-empty text values")),
                }
            }
            if required_non_empty && items.is_empty() {
                problems.push(format!("'{key}' must list at least one entry"));
            }
            out
        }
        Some(_) => {
            problems.push(format!("'{key}' must be a list"));
            Vec::new()
        }
        None if required_non_empty || key == "phonemizers" => {
            problems.push(format!("required field '{key}' is missing"));
            Vec::new()
        }
        None => Vec::new(),
    }
}

fn int_in(object: &Value, path: &str, key: &str, range: std::ops::RangeInclusive<i64>, problems: &mut Vec<String>) -> Option<i64> {
    match object.get(key).and_then(Value::as_i64) {
        Some(n) if range.contains(&n) => Some(n),
        Some(n) => {
            problems.push(format!("'{path}.{key}' is {n}, expected {} to {}", range.start(), range.end()));
            None
        }
        None => {
            problems.push(format!("'{path}.{key}' is missing or not a whole number"));
            None
        }
    }
}

/// Every rule of the format, collected as readable lines. Empty means the manifest is valid.
pub fn validate(value: &Value) -> Vec<String> {
    let mut p = Vec::new();
    if !value.is_object() {
        return vec!["manifest.json must contain a JSON object".to_string()];
    }
    if let Some(format) = text(value, "format", &mut p) {
        if format != FORMAT_NAME {
            p.push(format!("'format' must be \"{FORMAT_NAME}\", found \"{format}\""));
        }
    }
    int_in(value, "", "formatVersion", 1..=u32::MAX as i64, &mut p);
    if let Some(id) = text(value, "voiceId", &mut p) {
        if !is_valid_voice_id(id) {
            p.push("'voiceId' must be 3-64 characters: lowercase letters, digits, '.', '_' or '-', starting with a letter or digit".into());
        }
    }
    if let Some(name) = text(value, "voiceName", &mut p) {
        if name.chars().count() > 100 {
            p.push("'voiceName' is longer than 100 characters".into());
        }
    }
    text(value, "author", &mut p);
    match value.get("description") {
        Some(Value::String(_)) => {}
        Some(_) => p.push("'description' must be text".into()),
        None => p.push("required field 'description' is missing".into()),
    }
    for lang in string_list(value, "languages", true, &mut p) {
        if !is_language_tag(&lang) {
            p.push(format!("'languages' has an invalid language code: \"{lang}\" (use codes like ja, en, ru, zh-CN)"));
        }
    }
    for kind in string_list(value, "synthesisTypes", true, &mut p) {
        if !SYNTHESIS_TYPES.contains(&kind.as_str()) {
            p.push(format!("'synthesisTypes' has an unknown type \"{kind}\" (allowed: {})", SYNTHESIS_TYPES.join(", ")));
        }
    }
    string_list(value, "supportedBackends", true, &mut p);
    string_list(value, "phonemizers", false, &mut p);

    match value.get("audioConfiguration") {
        Some(cfg) if cfg.is_object() => {
            int_in(cfg, "audioConfiguration", "sampleRate", 8000..=192_000, &mut p);
            int_in(cfg, "audioConfiguration", "channels", 1..=2, &mut p);
            if let Some(bits) = int_in(cfg, "audioConfiguration", "bitDepth", 8..=32, &mut p) {
                if ![16, 24, 32].contains(&bits) {
                    p.push(format!("'audioConfiguration.bitDepth' is {bits}, expected 16, 24 or 32"));
                }
            }
        }
        _ => p.push("required object 'audioConfiguration' is missing".into()),
    }
    match value.get("pitchRange") {
        Some(range) if range.is_object() => {
            let min = int_in(range, "pitchRange", "minMidi", 0..=127, &mut p);
            let max = int_in(range, "pitchRange", "maxMidi", 0..=127, &mut p);
            if let (Some(min), Some(max)) = (min, max) {
                if min >= max {
                    p.push("'pitchRange.minMidi' must be lower than 'pitchRange.maxMidi'".into());
                }
            }
        }
        _ => p.push("required object 'pitchRange' is missing".into()),
    }
    match value.get("license") {
        Some(license) if license.is_object() => {
            if license.get("name").and_then(Value::as_str).map_or(true, |s| s.trim().is_empty()) {
                p.push("'license.name' is required (for example \"CC BY 4.0\" or \"Proprietary\")".into());
            }
        }
        _ => p.push("required object 'license' is missing".into()),
    }

    let files = string_list(value, "files", false, &mut p);
    if value.get("files").is_none() {
        p.push("required field 'files' is missing".into());
    }
    let mut file_set = BTreeSet::new();
    for file in &files {
        if let Err(why) = check_relative_path(file) {
            p.push(format!("'files' has an unsafe path: {why}"));
        } else if file == MANIFEST_NAME {
            p.push("'files' must not list manifest.json itself".into());
        } else if !file_set.insert(file.clone()) {
            p.push(format!("'files' lists '{file}' twice"));
        }
    }
    let lowered: BTreeSet<String> = files.iter().map(|f| f.to_lowercase()).collect();
    if lowered.len() != file_set.len() && file_set.len() == files.len() {
        p.push("'files' has two paths that differ only in letter case, which breaks on Windows and macOS".into());
    }
    match value.get("checksums") {
        Some(Value::Object(map)) => {
            for (path, sum) in map {
                let ok = sum.as_str().is_some_and(|s| s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
                if !ok {
                    p.push(format!("checksum of '{path}' must be a lowercase SHA-256 (64 hex characters)"));
                }
                if !file_set.contains(path) {
                    p.push(format!("'checksums' has an entry for '{path}', which is not in 'files'"));
                }
            }
            for file in &file_set {
                if !map.contains_key(file) {
                    p.push(format!("'{file}' is listed in 'files' but has no checksum"));
                }
            }
        }
        _ => p.push("required object 'checksums' is missing".into()),
    }
    for key in ["icon", "preview"] {
        match value.get(key) {
            None | Some(Value::Null) => {}
            Some(Value::String(path)) if file_set.contains(path) => {}
            Some(Value::String(path)) => p.push(format!("'{key}' points to '{path}', which is not in 'files'")),
            Some(_) => p.push(format!("'{key}' must be a path")),
        }
    }
    if let Some(deps) = value.get("dependencies") {
        match deps.as_array() {
            Some(list) => {
                for dep in list {
                    if !dep.get("voiceId").and_then(Value::as_str).is_some_and(is_valid_voice_id) {
                        p.push("every entry of 'dependencies' needs a valid 'voiceId'".into());
                    }
                }
            }
            None => p.push("'dependencies' must be a list".into()),
        }
    }
    p
}

/// Parses and validates `manifest.json`. A newer format version is reported separately, because such a
/// manifest may legitimately look different.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| EvbankError::Invalid(vec![format!("manifest.json is not valid JSON: {e}")]))?;
    if let Some(found) = value.get("formatVersion").and_then(Value::as_u64) {
        if found > FORMAT_VERSION as u64 {
            return Err(EvbankError::UnsupportedVersion { found: found as u32, supported: FORMAT_VERSION });
        }
    }
    let mut problems = validate(&value);
    if problems.len() > 30 {
        problems.truncate(30);
        problems.push("... and more".into());
    }
    if !problems.is_empty() {
        return Err(EvbankError::Invalid(problems));
    }
    serde_json::from_value(value).map_err(|e| EvbankError::Invalid(vec![e.to_string()]))
}

#[cfg(test)]
pub fn sample_manifest_json() -> Value {
    serde_json::json!({
        "format": "evbank",
        "formatVersion": 1,
        "voiceId": "test-voice",
        "voiceName": "Test Voice",
        "version": "1.0.0",
        "author": "Tester",
        "description": "A synthetic test voice.",
        "languages": ["en"],
        "synthesisTypes": ["sample", "singing"],
        "supportedBackends": ["eve-classic"],
        "phonemizers": ["english-basic"],
        "audioConfiguration": { "sampleRate": 22050, "channels": 1, "bitDepth": 16 },
        "pitchRange": { "minMidi": 48, "maxMidi": 72 },
        "license": { "name": "CC0-1.0" },
        "files": [],
        "checksums": {}
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_valid() {
        assert_eq!(validate(&sample_manifest_json()), Vec::<String>::new());
        assert!(parse_manifest(sample_manifest_json().to_string().as_bytes()).is_ok());
    }

    #[test]
    fn reports_every_missing_required_field() {
        for key in [
            "format", "formatVersion", "voiceId", "voiceName", "author", "description", "languages", "synthesisTypes",
            "supportedBackends", "phonemizers", "audioConfiguration", "pitchRange", "license", "files", "checksums",
        ] {
            let mut value = sample_manifest_json();
            value.as_object_mut().unwrap().remove(key);
            let problems = validate(&value);
            assert!(problems.iter().any(|line| line.contains(key)), "removing {key} should mention it, got {problems:?}");
        }
    }

    #[test]
    fn rejects_bad_values() {
        let cases: Vec<(&str, Value)> = vec![
            ("voiceId", "../x".into()),
            ("voiceId", "Has Space".into()),
            ("languages", serde_json::json!(["not a lang"])),
            ("synthesisTypes", serde_json::json!(["magic"])),
            ("audioConfiguration", serde_json::json!({"sampleRate": 100, "channels": 1, "bitDepth": 16})),
            ("audioConfiguration", serde_json::json!({"sampleRate": 22050, "channels": 3, "bitDepth": 16})),
            ("audioConfiguration", serde_json::json!({"sampleRate": 22050, "channels": 1, "bitDepth": 12})),
            ("pitchRange", serde_json::json!({"minMidi": 72, "maxMidi": 48})),
            ("license", serde_json::json!({})),
            ("files", serde_json::json!(["../evil.wav"])),
            ("checksums", serde_json::json!({"a.wav": "short"})),
        ];
        for (key, bad) in cases {
            let mut value = sample_manifest_json();
            value[key] = bad;
            assert!(!validate(&value).is_empty(), "{key} should be rejected");
        }
    }

    #[test]
    fn files_and_checksums_must_match() {
        let mut value = sample_manifest_json();
        value["files"] = serde_json::json!(["Audio/a.wav"]);
        assert!(validate(&value).iter().any(|l| l.contains("no checksum")));
        value["checksums"] = serde_json::json!({ "Audio/a.wav": "0".repeat(64), "Audio/b.wav": "0".repeat(64) });
        assert!(validate(&value).iter().any(|l| l.contains("not in 'files'")));
    }

    #[test]
    fn case_only_duplicates_are_rejected() {
        let mut value = sample_manifest_json();
        value["files"] = serde_json::json!(["Audio/A.wav", "audio/a.wav"]);
        value["checksums"] = serde_json::json!({ "Audio/A.wav": "0".repeat(64), "audio/a.wav": "0".repeat(64) });
        assert!(validate(&value).iter().any(|l| l.contains("letter case")));
    }

    #[test]
    fn newer_format_version_is_reported_separately() {
        let mut value = sample_manifest_json();
        value["formatVersion"] = 99.into();
        assert!(matches!(
            parse_manifest(value.to_string().as_bytes()),
            Err(EvbankError::UnsupportedVersion { found: 99, supported: 1 })
        ));
    }

    #[test]
    fn broken_json_is_readable() {
        let err = parse_manifest(b"{ nope").unwrap_err().to_string();
        assert!(err.contains("not valid JSON"));
    }

    #[test]
    fn version_comparison() {
        assert!(version_at_least("1.2.0", "1.1.9"));
        assert!(version_at_least("1.0", "1.0.0"));
        assert!(!version_at_least("0.9.9", "1.0.0"));
        assert!(version_at_least("2.0.0-beta", "1.5"));
    }
}
