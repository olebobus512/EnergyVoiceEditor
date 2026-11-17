//! Rules for anything that comes from outside: archive entry names, manifest paths, sizes.

/// Limits that stop zip bombs and absurd archives. The defaults suit large sample banks.
#[derive(Clone, Debug)]
pub struct Limits {
    pub max_entries: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    /// Largest allowed uncompressed/compressed ratio for an entry bigger than `RATIO_MIN_BYTES`.
    pub max_ratio: u64,
}

pub const RATIO_MIN_BYTES: u64 = 1024 * 1024;
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MANIFEST_NAME: &str = "manifest.json";

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 50_000,
            max_total_bytes: 4 * 1024 * 1024 * 1024,
            max_file_bytes: 2 * 1024 * 1024 * 1024,
            max_ratio: 200,
        }
    }
}

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1",
    "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Checks one path component (a single file or folder name).
pub fn check_component(component: &str) -> std::result::Result<(), String> {
    if component.is_empty() {
        return Err("empty name".into());
    }
    if component == "." || component == ".." {
        return Err("'.' and '..' are not allowed".into());
    }
    if component.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err("contains a character that is not allowed".into());
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return Err("must not end with a dot or a space".into());
    }
    let stem = component.split('.').next().unwrap_or("").to_ascii_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err("is a reserved system name".into());
    }
    if component.len() > 120 {
        return Err("name is too long".into());
    }
    Ok(())
}

/// A relative path with `/` separators that cannot leave its folder and is valid on Windows, macOS and Linux.
pub fn check_relative_path(path: &str) -> std::result::Result<(), String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    if path.len() > 240 {
        return Err("path is too long".into());
    }
    if path.starts_with('/') {
        return Err("absolute paths are not allowed".into());
    }
    for component in path.split('/') {
        check_component(component).map_err(|why| format!("'{path}': {why}"))?;
    }
    Ok(())
}

/// Archives written on Windows sometimes use `\`. Entry names are normalised to `/` before they are checked.
pub fn normalize_entry_name(name: &str) -> String {
    name.replace('\\', "/")
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_odd_paths() {
        for bad in [
            "../evil", "a/../b", "/abs", "C:/x", "a//b", "a/./b", "a\\b", "nul.txt", "dir/CON", "a/b.", "a/b ", "x\0y", "a/b:stream",
            "",
        ] {
            assert!(check_relative_path(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn accepts_normal_paths() {
        for good in ["Audio/Samples/a_01.wav", "Config/pitch.json", "Resources/icon.png", "Dictionaries/русский.json"] {
            assert!(check_relative_path(good).is_ok(), "{good:?} should be accepted");
        }
    }

    #[test]
    fn normalizes_backslashes() {
        assert_eq!(normalize_entry_name("Audio\\a.wav"), "Audio/a.wav");
    }
}
