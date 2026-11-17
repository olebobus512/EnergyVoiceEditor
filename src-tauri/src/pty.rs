use portable_pty::{native_pty_system, CommandBuilder, PtySize, MasterPty, Child};
use std::io::{Read, Write};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

pub struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}
impl Drop for Session { fn drop(&mut self) { let _ = self.child.kill(); } }
#[derive(Default)]
pub struct PtyState(Mutex<Option<Session>>);
fn size(cols: u16, rows: u16) -> PtySize { PtySize { rows: rows.clamp(2, 300), cols: cols.clamp(10, 500), pixel_width: 0, pixel_height: 0 } }
#[tauri::command]
pub fn pty_start(app: AppHandle, state: State<'_, PtyState>, cols: u16, rows: u16) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    slot.take();
    let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| e.to_string())?;
    let mut command = CommandBuilder::new(if cfg!(windows) { "powershell.exe" } else { "/bin/bash" });
    if cfg!(windows) { command.args(["-NoLogo"]); }
    command.env("TERM", "xterm-256color");
    let child = pair.slave.spawn_command(command).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    *slot = Some(Session { master: pair.master, writer, child });
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop { match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => { let _ = app.emit("pty-data", buffer[..n].to_vec()); }
        }}
    });
    Ok(())
}
#[tauri::command]
pub fn pty_write(state: State<'_, PtyState>, data: String) -> Result<(), String> {
    if data.len() > 65536 { return Err("Terminal input too large".into()); }
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    let session = slot.as_mut().ok_or("Terminal is not running")?;
    session.writer.write_all(data.as_bytes()).and_then(|_| session.writer.flush()).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn pty_resize(state: State<'_, PtyState>, cols: u16, rows: u16) -> Result<(), String> {
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(session) = slot.as_ref() { session.master.resize(size(cols, rows)).map_err(|e| e.to_string())?; }
    Ok(())
}
pub fn shutdown(state: &PtyState) { if let Ok(mut slot) = state.0.lock() { slot.take(); } }

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn conpty_supports_interactive_input_and_resize() {
        let pair = native_pty_system().openpty(size(80, 24)).unwrap();
        let mut command = CommandBuilder::new("powershell.exe");
        command.args(["-NoLogo", "-NoProfile", "-Command", "[Console]::Write('EVE_READY'); $x = Read-Host; Write-Output ('EVE_REPLY_' + $x)"]);
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let mut session = Session { master: pair.master, writer, child };
        session.master.resize(size(100, 30)).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            while let Ok(n) = reader.read(&mut bytes) {
                if n == 0 || tx.send(bytes[..n].to_vec()).is_err() { break; }
            }
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut text = String::new(); let mut sent = false;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let bytes = rx.recv_timeout(remaining).unwrap_or_else(|_| panic!("PTY did not answer: {text:?}"));
            text.push_str(&String::from_utf8_lossy(&bytes));
            if text.contains("\x1b[6n") {
                session.writer.write_all(b"\x1b[1;1R").unwrap(); session.writer.flush().unwrap();
                text = text.replace("\x1b[6n", "");
            }
            if !sent && text.contains("EVE_READY") {
                session.writer.write_all(b"ping\r").unwrap(); session.writer.flush().unwrap(); sent = true;
            }
            if text.contains("EVE_REPLY_ping") { break; }
        }
    }
}
