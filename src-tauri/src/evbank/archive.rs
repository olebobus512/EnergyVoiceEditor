//! Safe reading and writing of voice bank archives and folders.
//!
//! Everything that comes from outside is untrusted: entry names are checked, links and encrypted entries are
//! refused, and sizes are enforced while data is written (a zip header can lie about the size).

use super::manifest::{parse_manifest, Manifest};
use super::safety::{check_component, check_relative_path, hex, normalize_entry_name, Limits, MANIFEST_NAME, MAX_MANIFEST_BYTES, RATIO_MIN_BYTES};
use super::{EvbankError, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Copies at most `limit` bytes. Returns an error if the source has more.
fn copy_limited(reader: &mut impl Read, writer: &mut impl Write, limit: u64) -> Result<u64> {
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        if total > limit {
            return Err(EvbankError::Unsafe("a file is larger than its declared size or the size limit (possible zip bomb)".into()));
        }
        writer.write_all(&buffer[..n])?;
    }
}

struct Entry {
    index: usize,
    /// Path inside the bank, with any wrapper folder removed.
    path: String,
    size: u64,
}

/// Unpacks `archive` into the (empty, existing) folder `dest`. Does not validate the manifest; call
/// `verify::verify_dir` afterwards.
pub fn extract_archive(archive: &Path, dest: &Path, limits: &Limits) -> Result<()> {
    let file = File::open(archive).map_err(|e| EvbankError::NotABank(format!("cannot open the file: {e}")))?;
    let mut zip = ZipArchive::new(BufReader::new(file)).map_err(|_| EvbankError::NotABank("the file is not a valid .evbank (zip) archive".into()))?;
    if zip.len() > limits.max_entries {
        return Err(EvbankError::Unsafe(format!("the archive has more than {} entries", limits.max_entries)));
    }

    let mut raw = Vec::new();
    let mut declared_total = 0u64;
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).map_err(|e| EvbankError::NotABank(format!("damaged archive: {e}")))?;
        let raw_name = entry.name().map_err(|_| EvbankError::Unsafe("an entry name in the archive is not valid text".into()))?;
        let name = normalize_entry_name(&raw_name);
        if name.starts_with("__MACOSX/") || name.ends_with(".DS_Store") {
            continue;
        }
        if entry.encrypted() {
            return Err(EvbankError::Unsafe(format!("'{name}' is encrypted; encrypted archives are not supported")));
        }
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(EvbankError::Unsafe(format!("'{name}' is a symbolic link")));
        }
        if entry.is_dir() || name.ends_with('/') {
            continue;
        }
        let size = entry.size();
        if size > limits.max_file_bytes {
            return Err(EvbankError::Unsafe(format!("'{name}' is larger than the {} MiB per-file limit", limits.max_file_bytes >> 20)));
        }
        if size > RATIO_MIN_BYTES && entry.compressed_size() > 0 && size / entry.compressed_size() > limits.max_ratio {
            return Err(EvbankError::Unsafe(format!("'{name}' has an extreme compression ratio (possible zip bomb)")));
        }
        declared_total = declared_total.saturating_add(size);
        if declared_total > limits.max_total_bytes {
            return Err(EvbankError::Unsafe(format!("the archive unpacks to more than {} MiB", limits.max_total_bytes >> 20)));
        }
        raw.push((index, name, size));
    }

    // The manifest sits either at the top or inside one wrapper folder (ExampleVoice/manifest.json).
    let prefix = if raw.iter().any(|(_, name, _)| name == MANIFEST_NAME) {
        String::new()
    } else {
        let mut wrappers: Vec<&str> = raw.iter().filter_map(|(_, name, _)| name.strip_suffix(&format!("/{MANIFEST_NAME}"))).collect();
        wrappers.retain(|dir| !dir.contains('/'));
        match wrappers.as_slice() {
            [dir] => format!("{dir}/"),
            _ => return Err(EvbankError::NotABank("manifest.json was not found in the archive".into())),
        }
    };

    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, name, size) in raw {
        let Some(path) = name.strip_prefix(&prefix) else {
            return Err(EvbankError::Invalid(vec![format!("'{name}' is outside the voice bank folder '{}'", prefix.trim_end_matches('/'))]));
        };
        check_relative_path(path).map_err(|why| EvbankError::Unsafe(format!("bad path in the archive: {why}")))?;
        if !seen.insert(path.to_lowercase()) {
            return Err(EvbankError::Invalid(vec![format!("'{path}' appears twice (names that differ only in letter case count as the same)")]));
        }
        entries.push(Entry { index, path: path.to_string(), size });
    }

    let mut written_total = 0u64;
    for entry in entries {
        let target = dest.join(entry.path.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut source = zip.by_index(entry.index).map_err(|e| EvbankError::NotABank(format!("damaged archive: {e}")))?;
        let mut out = BufWriter::new(File::create(&target)?);
        let limit = entry.size.min(limits.max_file_bytes);
        let written = copy_limited(&mut source, &mut out, limit)?;
        out.flush()?;
        written_total += written;
        if written != entry.size {
            return Err(EvbankError::Corrupted(vec![format!("'{}' is shorter than the archive says", entry.path)]));
        }
        if written_total > limits.max_total_bytes {
            return Err(EvbankError::Unsafe("the archive unpacks to more data than the limit allows".into()));
        }
    }
    Ok(())
}

/// Copies a folder into `dest`, refusing links and unsafe names and applying the same limits.
pub fn copy_folder(src: &Path, dest: &Path, limits: &Limits) -> Result<()> {
    let meta = fs::symlink_metadata(src).map_err(|e| EvbankError::NotABank(format!("cannot read the folder: {e}")))?;
    if !meta.is_dir() {
        return Err(EvbankError::NotABank("this is not a folder".into()));
    }
    let mut count = 0usize;
    let mut total = 0u64;
    copy_tree(src, dest, limits, &mut count, &mut total)
}

fn copy_tree(src: &Path, dest: &Path, limits: &Limits, count: &mut usize, total: &mut u64) -> Result<()> {
    for item in fs::read_dir(src)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        check_component(&name).map_err(|why| EvbankError::Unsafe(format!("'{name}': {why}")))?;
        let meta = fs::symlink_metadata(item.path())?;
        let target = dest.join(&name);
        if meta.file_type().is_symlink() {
            return Err(EvbankError::Unsafe(format!("'{name}' is a link; links are not copied")));
        }
        if meta.is_dir() {
            fs::create_dir_all(&target)?;
            copy_tree(&item.path(), &target, limits, count, total)?;
        } else if meta.is_file() {
            *count += 1;
            *total += meta.len();
            if *count > limits.max_entries || meta.len() > limits.max_file_bytes || *total > limits.max_total_bytes {
                return Err(EvbankError::Unsafe("the folder is larger than the limits allow".into()));
            }
            fs::copy(item.path(), &target)?;
        }
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..n]);
    }
}

/// All files below `root` as sorted `/`-separated relative paths. Links are an error.
pub fn list_files(root: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
        for item in fs::read_dir(dir)? {
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            let meta = fs::symlink_metadata(item.path())?;
            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            if meta.file_type().is_symlink() {
                return Err(EvbankError::Unsafe(format!("'{rel}' is a link")));
            }
            if meta.is_dir() {
                walk(&item.path(), &rel, out)?;
            } else if meta.is_file() {
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, "", &mut out)?;
    out.sort();
    Ok(out)
}

fn zip_options(path: &str) -> SimpleFileOptions {
    // Models and compressed audio do not shrink; storing them keeps packing and installing fast.
    let stored = [".onnx", ".ogg", ".flac", ".png", ".jpg"].iter().any(|ext| path.to_lowercase().ends_with(ext));
    SimpleFileOptions::default()
        .compression_method(if stored { CompressionMethod::Stored } else { CompressionMethod::Deflated })
        .large_file(true)
}

fn write_zip(root: &Path, manifest_bytes: &[u8], files: &[String], out_path: &Path) -> Result<()> {
    let temp = out_path.with_extension("evbank.partial");
    let result = (|| -> Result<()> {
        let mut zip = ZipWriter::new(BufWriter::new(File::create(&temp)?));
        zip.start_file(MANIFEST_NAME, zip_options(MANIFEST_NAME)).map_err(|e| EvbankError::Io(e.to_string()))?;
        zip.write_all(manifest_bytes)?;
        for file in files {
            zip.start_file(file, zip_options(file)).map_err(|e| EvbankError::Io(e.to_string()))?;
            let mut source = BufReader::new(File::open(root.join(file.replace('/', std::path::MAIN_SEPARATOR_STR)))?);
            io::copy(&mut source, &mut zip)?;
        }
        zip.finish().map_err(|e| EvbankError::Io(e.to_string()))?.flush()?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            fs::rename(&temp, out_path).map_err(|e| {
                let _ = fs::remove_file(&temp);
                EvbankError::Io(format!("cannot write {}: {e}", out_path.display()))
            })
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

/// Builds a `.evbank` from a folder that holds a `manifest.json` and the bank's files. The `files` and
/// `checksums` fields are computed here, so authors only write the descriptive part of the manifest.
pub fn pack(src: &Path, out_path: &Path) -> Result<Manifest> {
    let manifest_path = src.join(MANIFEST_NAME);
    let meta = fs::metadata(&manifest_path).map_err(|_| EvbankError::NotABank("the folder has no manifest.json".into()))?;
    if meta.len() > MAX_MANIFEST_BYTES {
        return Err(EvbankError::Invalid(vec!["manifest.json is larger than 1 MiB".into()]));
    }
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)
        .map_err(|e| EvbankError::Invalid(vec![format!("manifest.json is not valid JSON: {e}")]))?;
    let out_abs = out_path.canonicalize().ok();
    let mut files = Vec::new();
    for path in list_files(src)? {
        if path == MANIFEST_NAME {
            continue;
        }
        if out_abs.is_some() && src.join(&path).canonicalize().ok() == out_abs {
            continue;
        }
        check_relative_path(&path).map_err(|why| EvbankError::Unsafe(format!("cannot pack: {why}")))?;
        files.push(path);
    }
    let mut checksums = serde_json::Map::new();
    for file in &files {
        checksums.insert(file.clone(), sha256_file(&src.join(file.replace('/', std::path::MAIN_SEPARATOR_STR)))?.into());
    }
    value["files"] = serde_json::json!(files);
    value["checksums"] = serde_json::Value::Object(checksums);
    let bytes = serde_json::to_vec_pretty(&value).map_err(|e| EvbankError::Io(e.to_string()))?;
    let manifest = parse_manifest(&bytes)?;
    write_zip(src, &bytes, &files, out_path)?;
    Ok(manifest)
}

/// Writes an already installed (or unpacked) bank back into a `.evbank` archive unchanged.
pub fn export_dir(root: &Path, out_path: &Path) -> Result<()> {
    let bytes = fs::read(root.join(MANIFEST_NAME))?;
    let manifest = parse_manifest(&bytes)?;
    write_zip(root, &bytes, &manifest.files, out_path)
}

pub fn unique_temp_dir(parent: &Path, label: &str) -> Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = parent.join(format!("{label}-{}-{nanos}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}
