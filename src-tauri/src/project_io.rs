use crate::settings::PROJECT_EXTENSION;
use std::path::{Path, PathBuf};

const MAX_PROJECT_BYTES: u64 = 100 * 1024 * 1024;

fn project_path(path: &str) -> Result<PathBuf, String> {
    if !path.to_lowercase().ends_with(PROJECT_EXTENSION) {
        return Err(format!("Project files must end with {PROJECT_EXTENSION}"));
    }
    Ok(PathBuf::from(path))
}

/// Writes to a temporary file first, so a failed write never replaces the previous project.
#[tauri::command]
pub fn save_project_file(path: String, content: String) -> Result<(), String> {
    let target = project_path(&path)?;
    if content.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project is too large to save".into());
    }
    write_atomic(&target, &content)
}

fn write_atomic(target: &Path, content: &str) -> Result<(), String> {
    let folder = target
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or("The project folder does not exist")?;
    let temp = folder.join(format!(
        ".{}.tmp",
        target.file_name().map(|name| name.to_string_lossy()).unwrap_or_default()
    ));
    std::fs::write(&temp, content).map_err(|e| format!("Cannot write project: {e}"))?;
    std::fs::rename(&temp, target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("Cannot save project: {e}")
    })
}

#[tauri::command]
pub fn open_project_file(path: String) -> Result<String, String> {
    let source = project_path(&path)?;
    let meta = std::fs::metadata(&source).map_err(|e| format!("Cannot open project: {e}"))?;
    if !meta.is_file() {
        return Err("Selected path is not a file".into());
    }
    if meta.len() > MAX_PROJECT_BYTES {
        return Err("Project file is too large".into());
    }
    std::fs::read_to_string(&source).map_err(|e| format!("Cannot read project: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("eve-project-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_open_round_trips() {
        let dir = temp_dir("roundtrip");
        let file = dir.join("song.eveproj");
        let path = file.to_string_lossy().into_owned();
        save_project_file(path.clone(), "{\"version\":1}".into()).unwrap();
        assert_eq!(open_project_file(path).unwrap(), "{\"version\":1}");
        assert!(!dir.join(".song.eveproj.tmp").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_wrong_extension_and_missing_folder() {
        let dir = temp_dir("bad");
        assert!(save_project_file(dir.join("x.txt").to_string_lossy().into_owned(), "{}".into()).is_err());
        let missing = dir.join("nope").join("x.eveproj");
        assert!(save_project_file(missing.to_string_lossy().into_owned(), "{}".into()).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
