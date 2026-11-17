use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, State};

const DONE_MARK: &str = "__EVE_DONE__";
const MAX_COMMAND_BYTES: usize = 8000;

/// One long-lived PowerShell process. Working directory and variables persist between commands, like a terminal.
pub struct Session {
    child: Child,
    stdin: ChildStdin,
}

#[derive(Default)]
pub struct ShellState(pub Arc<Mutex<Option<Session>>>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShellLine {
    pub stream: &'static str,
    pub text: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShellDone {
    pub id: u64,
}

fn program() -> &'static str {
    if cfg!(windows) {
        "powershell.exe"
    } else {
        "pwsh"
    }
}

fn start(app: &AppHandle) -> Result<Session, String> {
    let mut command = Command::new(program());
    command
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        command.current_dir(home);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start {}: {e}", program()))?;
    let mut stdin = child.stdin.take().ok_or("PowerShell stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("PowerShell stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("PowerShell stderr unavailable")?;

    // Make output UTF-8 so Cyrillic and Japanese text is readable in the console.
    let init = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; $OutputEncoding = [Text.Encoding]::UTF8\n";
    stdin
        .write_all(init.as_bytes())
        .and_then(|_| stdin.flush())
        .map_err(|e| format!("Cannot initialise PowerShell: {e}"))?;

    let out_app = app.clone();
    thread::spawn(move || read_stdout(stdout, out_app));
    let err_app = app.clone();
    thread::spawn(move || read_lines(stderr, "stderr", err_app));
    Ok(Session { child, stdin })
}

fn read_lines(reader: impl Read, stream: &'static str, app: AppHandle) {
    let mut reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buffer).trim_end().to_string();
                let _ = app.emit("shell-output", ShellLine { stream, text });
            }
        }
    }
}

/// Reads stdout and turns the end-of-command marker into a `shell-done` event instead of showing it.
fn read_stdout(reader: impl Read, app: AppHandle) {
    let mut reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buffer).trim_end().to_string();
                if let Some(position) = text.find(DONE_MARK) {
                    let id = text[position + DONE_MARK.len()..].trim().parse().unwrap_or(0);
                    let _ = app.emit("shell-done", ShellDone { id });
                } else {
                    let _ = app.emit("shell-output", ShellLine { stream: "stdout", text });
                }
            }
        }
    }
}

/// Runs one command inside the persistent session. The command travels as base64 so its encoding survives the pipe.
fn send(session: &mut Session, id: u64, command: &str) -> Result<(), String> {
    let encoded = base64_encode(command.as_bytes());
    let script = format!(
        "Invoke-Expression ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')))\nWrite-Output '{DONE_MARK}{id}'\n"
    );
    session
        .stdin
        .write_all(script.as_bytes())
        .and_then(|_| session.stdin.flush())
        .map_err(|e| format!("Cannot send command to PowerShell: {e}"))
}

#[tauri::command]
pub fn shell_run(app: AppHandle, state: State<'_, ShellState>, id: u64, command: String) -> Result<(), String> {
    if command.trim().is_empty() {
        return Err("Command is empty".into());
    }
    if command.len() > MAX_COMMAND_BYTES {
        return Err(format!("Command is longer than {MAX_COMMAND_BYTES} bytes"));
    }
    let mut guard = state.0.lock().map_err(|_| "Shell lock poisoned")?;
    let alive = match guard.as_mut() {
        Some(session) => matches!(session.child.try_wait(), Ok(None)),
        None => false,
    };
    if !alive {
        *guard = Some(start(&app)?);
    }
    let session = guard.as_mut().ok_or("PowerShell is not running")?;
    send(session, id, &command)
}

#[tauri::command]
pub fn shell_restart(app: AppHandle, state: State<'_, ShellState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|_| "Shell lock poisoned")?;
    if let Some(mut session) = guard.take() {
        let _ = session.child.kill();
        let _ = session.child.wait();
    }
    *guard = Some(start(&app)?);
    Ok(())
}

pub fn shutdown_shell(state: &ShellState) {
    if let Ok(mut guard) = state.0.lock() {
        if let Some(mut session) = guard.take() {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode("Привет".as_bytes()), "0J/RgNC40LLQtdGC");
    }
}
