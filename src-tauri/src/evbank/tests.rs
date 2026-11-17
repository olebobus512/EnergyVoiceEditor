//! End-to-end tests of the voice bank pipeline: pack, install, verify, and a set of hostile archives.

use super::archive::{copy_folder, pack};
use super::manifest::{sample_manifest_json, IMPLEMENTED_BACKENDS};
use super::safety::Limits;
use super::store::{Status, VoicebankStore};
use super::wav::make_wav;
use super::EvbankError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("eve-evbank-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Sandbox(path)
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sha(bytes: &[u8]) -> String {
    super::safety::hex(&Sha256::digest(bytes))
}

/// A bank source folder that `pack` can turn into an archive.
fn bank_source(dir: &Path, id: &str, version: &str, extra: impl FnOnce(&mut Value)) {
    fs::create_dir_all(dir.join("Audio/Samples")).unwrap();
    fs::create_dir_all(dir.join("Resources")).unwrap();
    fs::write(dir.join("Audio/Samples/a.wav"), make_wav(22050, 1, 16, 2205)).unwrap();
    fs::write(dir.join("Resources/preview.wav"), make_wav(44100, 2, 16, 441)).unwrap();
    let mut manifest = sample_manifest_json();
    manifest["voiceId"] = id.into();
    manifest["version"] = version.into();
    manifest["preview"] = "Resources/preview.wav".into();
    extra(&mut manifest);
    fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn packed(sb: &Sandbox, id: &str, version: &str) -> PathBuf {
    let src = sb.path(&format!("src-{id}-{version}"));
    bank_source(&src, id, version, |_| {});
    let out = sb.path(&format!("{id}-{version}.evbank"));
    pack(&src, &out).unwrap();
    out
}

/// Builds an archive by hand, with exact control over names, bytes and the manifest.
fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn manifest_for(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut manifest = sample_manifest_json();
    manifest["files"] = json!(files.iter().map(|(n, _)| *n).collect::<Vec<_>>());
    manifest["checksums"] = Value::Object(files.iter().map(|(n, b)| (n.to_string(), json!(sha(b)))).collect());
    serde_json::to_vec(&manifest).unwrap()
}

fn store(sb: &Sandbox) -> VoicebankStore {
    VoicebankStore::new(sb.path("store")).unwrap()
}

fn assert_no_leftovers(store: &VoicebankStore) {
    for hidden in [".staging", ".trash"] {
        let dir = store.root().join(hidden);
        if dir.exists() {
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 0, "{hidden} should be empty");
        }
    }
}

#[test]
fn pack_install_list_verify_export_roundtrip() {
    let sb = Sandbox::new("roundtrip");
    let archive = packed(&sb, "test-voice", "1.2.0");
    let store = store(&sb);

    let outcome = store.install_archive(&archive, false).unwrap();
    assert_eq!((outcome.id.as_str(), outcome.version.as_str(), outcome.replaced), ("test-voice", "1.2.0", None));

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, Status::Ready);
    assert_eq!(list[0].name, "Test Voice");
    assert!(list[0].has_preview);
    assert!(list[0].size_bytes > 0);
    // No synthesis backend exists yet, so the bank is valid but cannot be used. The UI must say so.
    assert_eq!(list[0].usable_backends, IMPLEMENTED_BACKENDS.iter().map(|s| s.to_string()).collect::<Vec<_>>());

    let verified = store.verify("test-voice").unwrap();
    assert_eq!(verified.file_count, 2);
    assert!(store.preview_path("test-voice").unwrap().is_some());

    let exported = sb.path("exported.evbank");
    store.export("test-voice", &exported).unwrap();
    let other = VoicebankStore::new(sb.path("store2")).unwrap();
    assert_eq!(other.install_archive(&exported, false).unwrap().id, "test-voice");
    assert_no_leftovers(&store);
}

#[test]
fn install_from_folder_works() {
    let sb = Sandbox::new("folder");
    let src = sb.path("src");
    bank_source(&src, "folder-voice", "1.0.0", |_| {});
    let out = sb.path("tmp.evbank");
    pack(&src, &out).unwrap();
    // Unpack by installing, then install the unpacked folder into a second store.
    let first = store(&sb);
    first.install_archive(&out, false).unwrap();
    let second = VoicebankStore::new(sb.path("store-b")).unwrap();
    let outcome = second.install_folder(&sb.path("store/folder-voice"), false).unwrap();
    assert_eq!(outcome.id, "folder-voice");
    assert_eq!(second.list()[0].status, Status::Ready);
}

#[test]
fn archive_with_a_wrapper_folder_is_accepted() {
    let sb = Sandbox::new("wrapper");
    let audio = make_wav(22050, 1, 16, 100);
    let files: [(&str, &[u8]); 1] = [("Audio/a.wav", &audio)];
    let manifest = manifest_for(&files);
    let path = sb.path("w.evbank");
    make_zip(&path, &[("ExampleVoice/manifest.json", &manifest), ("ExampleVoice/Audio/a.wav", &audio)]);
    assert!(store(&sb).install_archive(&path, false).is_ok());
}

#[test]
fn path_traversal_is_rejected_and_nothing_escapes() {
    let sb = Sandbox::new("traversal");
    let audio = make_wav(22050, 1, 16, 100);
    let files: [(&str, &[u8]); 1] = [("Audio/a.wav", &audio)];
    let manifest = manifest_for(&files);
    for evil in ["../evil.txt", "Audio/../../evil.txt", "/abs/evil.txt", "C:/evil.txt", "Audio\\..\\..\\evil.txt"] {
        let path = sb.path("evil.evbank");
        make_zip(&path, &[("manifest.json", &manifest), ("Audio/a.wav", &audio), (evil, b"x")]);
        let s = store(&sb);
        let err = s.install_archive(&path, false).unwrap_err();
        assert!(matches!(err, EvbankError::Unsafe(_) | EvbankError::Invalid(_)), "{evil}: {err}");
        assert!(!sb.path("evil.txt").exists() && !sb.0.parent().unwrap().join("evil.txt").exists());
        assert!(s.list().is_empty());
        assert_no_leftovers(&s);
    }
}

#[test]
fn manifest_path_traversal_is_rejected() {
    let sb = Sandbox::new("manifest-traversal");
    let mut manifest = sample_manifest_json();
    manifest["files"] = json!(["../../etc/passwd"]);
    manifest["checksums"] = json!({ "../../etc/passwd": "0".repeat(64) });
    let path = sb.path("m.evbank");
    make_zip(&path, &[("manifest.json", &serde_json::to_vec(&manifest).unwrap())]);
    assert!(matches!(store(&sb).install_archive(&path, false), Err(EvbankError::Invalid(_))));
}

#[cfg(unix)]
#[test]
fn symlink_entries_and_folders_are_rejected() {
    let sb = Sandbox::new("symlink");
    let manifest = manifest_for(&[]);
    let path = sb.path("l.evbank");
    {
        let mut zip = ZipWriter::new(fs::File::create(&path).unwrap());
        zip.start_file("manifest.json", SimpleFileOptions::default()).unwrap();
        zip.write_all(&manifest).unwrap();
        zip.add_symlink("link", "/etc/passwd", SimpleFileOptions::default()).unwrap();
        zip.finish().unwrap();
    }
    assert!(matches!(store(&sb).install_archive(&path, false), Err(EvbankError::Unsafe(_))));

    let dir = sb.path("with-link");
    fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink("/etc", dir.join("etc")).unwrap();
    let err = copy_folder(&dir, &sb.path("copy-target"), &Limits::default());
    assert!(matches!(err, Err(EvbankError::Unsafe(_))));
}

#[test]
fn zip_bombs_are_rejected() {
    let sb = Sandbox::new("bomb");
    // 4 MiB of zeros deflates to a few KiB: a ratio far above the limit.
    let zeros = vec![0u8; 4 * 1024 * 1024];
    let files: [(&str, &[u8]); 1] = [("Audio/big.bin", &zeros)];
    let manifest = manifest_for(&files);
    let path = sb.path("bomb.evbank");
    make_zip(&path, &[("manifest.json", &manifest), ("Audio/big.bin", &zeros)]);
    let s = store(&sb);
    assert!(matches!(s.install_archive(&path, false), Err(EvbankError::Unsafe(msg)) if msg.contains("compression ratio")));

    // Total and per-file limits are enforced too.
    let limited = VoicebankStore::with_limits(sb.path("store-limited"), Limits { max_total_bytes: 1000, ..Limits::default() }).unwrap();
    let noisy: Vec<u8> = (0..5000u32).map(|i| (i * 7919 % 251) as u8).collect();
    let files2: [(&str, &[u8]); 1] = [("Audio/n.bin", &noisy)];
    let manifest2 = manifest_for(&files2);
    let path2 = sb.path("big.evbank");
    make_zip(&path2, &[("manifest.json", &manifest2), ("Audio/n.bin", &noisy)]);
    assert!(matches!(limited.install_archive(&path2, false), Err(EvbankError::Unsafe(_))));
    let few = VoicebankStore::with_limits(sb.path("store-few"), Limits { max_entries: 1, ..Limits::default() }).unwrap();
    assert!(matches!(few.install_archive(&path2, false), Err(EvbankError::Unsafe(_))));
    assert_no_leftovers(&s);
    assert_no_leftovers(&limited);
}

#[test]
fn damaged_and_foreign_files_are_rejected() {
    let sb = Sandbox::new("damaged");
    let s = store(&sb);
    fs::write(sb.path("not.evbank"), b"this is not a zip").unwrap();
    assert!(matches!(s.install_archive(&sb.path("not.evbank"), false), Err(EvbankError::NotABank(_))));
    assert!(matches!(s.install_archive(&sb.path("missing.evbank"), false), Err(EvbankError::NotABank(_))));

    let good = packed(&sb, "cut-voice", "1.0.0");
    let bytes = fs::read(&good).unwrap();
    fs::write(sb.path("cut.evbank"), &bytes[..bytes.len() / 2]).unwrap();
    assert!(s.install_archive(&sb.path("cut.evbank"), false).is_err());

    make_zip(&sb.path("nomanifest.evbank"), &[("readme.txt", b"hi")]);
    assert!(matches!(s.install_archive(&sb.path("nomanifest.evbank"), false), Err(EvbankError::NotABank(_))));
    assert!(s.list().is_empty());
    assert_no_leftovers(&s);
}

#[test]
fn tampered_files_undeclared_files_and_bad_audio_are_caught() {
    let sb = Sandbox::new("tamper");
    let s = store(&sb);
    let audio = make_wav(22050, 1, 16, 100);
    let files: [(&str, &[u8]); 1] = [("Audio/a.wav", &audio)];
    let manifest = manifest_for(&files);

    // Content changed after the checksum was written.
    let mut changed = audio.clone();
    *changed.last_mut().unwrap() ^= 0xFF;
    make_zip(&sb.path("t1.evbank"), &[("manifest.json", &manifest), ("Audio/a.wav", &changed)]);
    let err = s.install_archive(&sb.path("t1.evbank"), false).unwrap_err();
    assert!(matches!(&err, EvbankError::Corrupted(l) if l[0].contains("checksum")), "{err}");

    // A file smuggled in without being declared.
    make_zip(&sb.path("t2.evbank"), &[("manifest.json", &manifest), ("Audio/a.wav", &audio), ("payload.exe", b"MZ")]);
    let err = s.install_archive(&sb.path("t2.evbank"), false).unwrap_err();
    assert!(matches!(&err, EvbankError::Invalid(l) if l[0].contains("payload.exe")), "{err}");

    // A declared file that is missing.
    make_zip(&sb.path("t3.evbank"), &[("manifest.json", &manifest)]);
    assert!(matches!(s.install_archive(&sb.path("t3.evbank"), false), Err(EvbankError::Corrupted(_))));

    // A WAV whose sample rate disagrees with the manifest.
    let wrong = make_wav(44100, 1, 16, 100);
    let files4: [(&str, &[u8]); 1] = [("Audio/a.wav", &wrong)];
    make_zip(&sb.path("t4.evbank"), &[("manifest.json", &manifest_for(&files4)), ("Audio/a.wav", &wrong)]);
    let err = s.install_archive(&sb.path("t4.evbank"), false).unwrap_err();
    assert!(matches!(&err, EvbankError::Corrupted(l) if l[0].contains("44100")), "{err}");

    // "WAV" that is not audio at all.
    let junk: &[u8] = b"RIFFnotreallyawavefile";
    let files5: [(&str, &[u8]); 1] = [("Audio/a.wav", junk)];
    make_zip(&sb.path("t5.evbank"), &[("manifest.json", &manifest_for(&files5)), ("Audio/a.wav", junk)]);
    assert!(matches!(s.install_archive(&sb.path("t5.evbank"), false), Err(EvbankError::Corrupted(_))));

    assert!(s.list().is_empty());
    assert_no_leftovers(&s);
}

#[test]
fn installed_bank_that_changes_on_disk_is_detected() {
    let sb = Sandbox::new("bitrot");
    let s = store(&sb);
    s.install_archive(&packed(&sb, "rot-voice", "1.0.0"), false).unwrap();
    let file = sb.path("store/rot-voice/Audio/Samples/a.wav");
    let mut bytes = fs::read(&file).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&file, bytes).unwrap();
    assert!(matches!(s.verify("rot-voice"), Err(EvbankError::Corrupted(_))));
    assert!(s.export("rot-voice", &sb.path("out.evbank")).is_err(), "a damaged bank must not be exported");

    fs::remove_file(&file).unwrap();
    assert_eq!(s.list()[0].status, Status::Corrupted);
}

#[test]
fn reinstalling_needs_confirmation_and_failure_keeps_the_old_bank() {
    let sb = Sandbox::new("replace");
    let s = store(&sb);
    s.install_archive(&packed(&sb, "twice", "1.0.0"), false).unwrap();
    let newer = packed(&sb, "twice", "2.0.0");

    let err = s.install_archive(&newer, false).unwrap_err();
    assert!(matches!(&err, EvbankError::AlreadyInstalled { installed, incoming, .. } if installed == "1.0.0" && incoming == "2.0.0"));
    assert_eq!(s.list()[0].version, "1.0.0");

    let outcome = s.install_archive(&newer, true).unwrap();
    assert_eq!(outcome.replaced.as_deref(), Some("1.0.0"));
    assert_eq!(s.list()[0].version, "2.0.0");

    // A broken replacement must leave the working bank exactly as it was.
    let audio = make_wav(22050, 1, 16, 100);
    let mut broken_manifest: Value = serde_json::from_slice(&manifest_for(&[("Audio/a.wav", &audio)])).unwrap();
    broken_manifest["voiceId"] = "twice".into();
    broken_manifest["version"] = "3.0.0".into();
    let mut changed = audio.clone();
    *changed.last_mut().unwrap() ^= 0xFF;
    make_zip(&sb.path("bad.evbank"), &[("manifest.json", &serde_json::to_vec(&broken_manifest).unwrap()), ("Audio/a.wav", &changed)]);
    assert!(s.install_archive(&sb.path("bad.evbank"), true).is_err());
    assert_eq!(s.list()[0].version, "2.0.0");
    assert_eq!(s.list()[0].status, Status::Ready);
    s.verify("twice").unwrap();
    assert_no_leftovers(&s);
}

#[test]
fn dependencies_set_the_status() {
    let sb = Sandbox::new("deps");
    let s = store(&sb);
    let src = sb.path("src-child");
    bank_source(&src, "child-voice", "1.0.0", |m| {
        m["dependencies"] = json!([{ "voiceId": "base-voice", "minVersion": "2.0.0" }]);
    });
    let child = sb.path("child.evbank");
    pack(&src, &child).unwrap();
    s.install_archive(&child, false).unwrap();
    assert_eq!(s.list()[0].status, Status::MissingDependencies);
    assert!(s.list()[0].problems[0].contains("base-voice"));

    s.install_archive(&packed(&sb, "base-voice", "1.0.0"), false).unwrap();
    let child_info = s.list().into_iter().find(|b| b.id == "child-voice").unwrap();
    assert_eq!(child_info.status, Status::MissingDependencies);
    assert!(child_info.problems[0].contains("2.0.0"));

    s.install_archive(&packed(&sb, "base-voice", "2.1.0"), true).unwrap();
    assert_eq!(s.list().into_iter().find(|b| b.id == "child-voice").unwrap().status, Status::Ready);
}

#[test]
fn newer_format_is_refused_and_marked_incompatible() {
    let sb = Sandbox::new("future");
    let s = store(&sb);
    let mut manifest = sample_manifest_json();
    manifest["formatVersion"] = 7.into();
    let path = sb.path("future.evbank");
    make_zip(&path, &[("manifest.json", &serde_json::to_vec(&manifest).unwrap())]);
    assert!(matches!(s.install_archive(&path, false), Err(EvbankError::UnsupportedVersion { found: 7, .. })));

    // A bank like that already sitting in the store (for example from a newer app version).
    let dir = sb.path("store/future-voice");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(s.list()[0].status, Status::Incompatible);
}

#[test]
fn uninstall_and_hostile_ids() {
    let sb = Sandbox::new("uninstall");
    let s = store(&sb);
    s.install_archive(&packed(&sb, "gone-soon", "1.0.0"), false).unwrap();
    s.uninstall("gone-soon").unwrap();
    assert!(s.list().is_empty());
    assert!(matches!(s.uninstall("gone-soon"), Err(EvbankError::NotInstalled(_))));
    for id in ["../store", "..", "a/b", ".staging", "UPPER", ""] {
        assert!(matches!(s.uninstall(id), Err(EvbankError::Invalid(_))), "{id:?}");
        assert!(s.verify(id).is_err());
    }
    assert_no_leftovers(&s);
}

#[test]
fn pack_reports_bad_manifests_and_leaves_no_partial_file() {
    let sb = Sandbox::new("pack-bad");
    let src = sb.path("src");
    bank_source(&src, "x", "1.0.0", |_| {}); // id too short
    let out = sb.path("out.evbank");
    assert!(matches!(pack(&src, &out), Err(EvbankError::Invalid(_))));
    assert!(!out.exists() && !sb.path("out.evbank.partial").exists());
    assert!(matches!(pack(&sb.path("nothing"), &out), Err(EvbankError::NotABank(_))));
}

#[test]
fn stale_staging_is_cleaned_on_start() {
    let sb = Sandbox::new("stale");
    let root = sb.path("store");
    fs::create_dir_all(root.join(".staging/crashed")).unwrap();
    fs::write(root.join(".staging/crashed/x"), b"x").unwrap();
    let s = VoicebankStore::new(root).unwrap();
    assert_no_leftovers(&s);
}

/// `Documentation/examples/ExampleTone.evbank` is written by `tools/make_example_evbank.py`, code that shares nothing
/// with the Rust packer. If this test passes, the format description is precise enough to implement twice.
#[test]
fn example_bank_written_by_the_python_tool_installs() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Documentation/examples/ExampleTone.evbank");
    let sb = Sandbox::new("example");
    let s = store(&sb);
    let outcome = s.install_archive(&fixture, false).unwrap();
    assert_eq!(outcome.id, "example-tone");
    let info = &s.list()[0];
    assert_eq!(info.status, Status::Ready);
    assert!(info.has_preview);
    assert_eq!(s.verify("example-tone").unwrap().file_count, 7);
}
