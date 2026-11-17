use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// How long the app waits for the worker to answer `shutdown` before it kills the process.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Owns the Python runtime process and speaks the JSON-lines protocol over its stdin/stdout.
pub struct PythonWorker {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl PythonWorker {
    /// `on_log` receives every stderr line of the worker (tracebacks, diagnostics).
    pub fn spawn(
        python: &str,
        script: &Path,
        on_log: impl Fn(String) + Send + 'static,
    ) -> Result<Self, String> {
        let mut command = Command::new(python);
        command
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to start Python worker '{python}': {e}"))?;

        let stdin = child.stdin.take().ok_or("Worker stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("Worker stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("Worker stderr unavailable")?;

        let (tx, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                on_log(line);
            }
        });

        Ok(Self { child, stdin, lines, next_id: 0 })
    }

    /// Sends a request and waits for its response. `on_event` receives progress notifications.
    pub fn request(
        &mut self,
        method: &str,
        params: Value,
        mut on_event: impl FnMut(&Value),
    ) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let message = json!({ "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{message}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Failed to write to Python worker: {e}"))?;

        loop {
            let line = self
                .lines
                .recv()
                .map_err(|_| "Python worker exited unexpectedly".to_string())?;
            let value: Value = serde_json::from_str(&line)
                .map_err(|e| format!("Invalid message from Python worker: {e}"))?;
            if value.get("event").is_some() {
                on_event(&value);
                continue;
            }
            if value["id"] != json!(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(error["message"].as_str().unwrap_or("Unknown worker error").to_string());
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Asks the worker to stop and waits at most `SHUTDOWN_GRACE`; a worker that hangs is killed.
    /// This must never block forever, otherwise the app cannot exit.
    pub fn shutdown(&mut self) {
        let message = json!({ "id": 0, "method": "shutdown", "params": {} });
        let sent = writeln!(self.stdin, "{message}").and_then(|_| self.stdin.flush()).is_ok();
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while sent {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(remaining) {
                Ok(line) => {
                    let answered = serde_json::from_str::<Value>(&line)
                        .map(|value| value["id"] == json!(0))
                        .unwrap_or(false);
                    if answered {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PythonWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python() -> Option<&'static str> {
        ["python3", "python"]
            .into_iter()
            .find(|name| Command::new(name).arg("--version").output().is_ok_and(|out| out.status.success()))
    }

    fn script(name: &str, body: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("eve-worker-{}-{name}.py", std::process::id()));
        std::fs::write(&path, body).unwrap();
        path
    }

    /// A worker that never answers must not keep the app from exiting.
    #[test]
    fn shutdown_kills_a_hung_worker() {
        let Some(python) = python() else { return };
        let path = script("hung", "import time\ntime.sleep(60)\n");
        let mut worker = PythonWorker::spawn(python, &path, |_| {}).unwrap();
        let started = Instant::now();
        worker.shutdown();
        assert!(started.elapsed() < Duration::from_secs(6), "shutdown took {:?}", started.elapsed());
        assert!(matches!(worker.child.try_wait(), Ok(Some(_))), "worker process is still running");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn shutdown_is_quick_for_a_well_behaved_worker() {
        let Some(python) = python() else { return };
        let path = script(
            "polite",
            "import sys, json\nfor line in sys.stdin:\n    req = json.loads(line)\n    print(json.dumps({'id': req['id'], 'result': {}}), flush=True)\n    break\n",
        );
        let mut worker = PythonWorker::spawn(python, &path, |_| {}).unwrap();
        let started = Instant::now();
        worker.shutdown();
        assert!(started.elapsed() < Duration::from_secs(1));
        let _ = std::fs::remove_file(path);
    }
}
