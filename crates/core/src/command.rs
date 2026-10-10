use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Tool {
    Adb,
    Fastboot,
    External(PathBuf),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invocation {
    pub tool: Tool,
    pub args: Vec<String>,
    pub directory: Option<PathBuf>,
    pub label: String,
    pub timeout: u64,
    #[serde(default)]
    pub needs_secret: bool,
    #[serde(default)]
    pub critical: bool,
    #[serde(skip)]
    pub secret: Option<SecretInput>,
}
#[derive(Clone)]
pub struct SecretInput(Arc<zeroize::Zeroizing<String>>);
impl std::fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted secret]")
    }
}
impl SecretInput {
    pub fn new(value: String) -> Self {
        Self(Arc::new(zeroize::Zeroizing::new(value)))
    }
    fn value(&self) -> &str {
        self.0.as_str()
    }
}
impl Invocation {
    pub fn adb(serial: &str, args: &[&str], label: &str) -> Self {
        Self {
            tool: Tool::Adb,
            args: [
                vec!["-s".into(), serial.into()],
                args.iter().map(|s| (*s).into()).collect(),
            ]
            .concat(),
            directory: None,
            label: label.into(),
            timeout: 300,
            needs_secret: false,
            critical: false,
            secret: None,
        }
    }
    pub fn fastboot(serial: &str, args: &[&str], label: &str) -> Self {
        let mut call = Self::adb(serial, args, label);
        call.tool = Tool::Fastboot;
        call
    }
    pub fn global(tool: Tool, args: &[&str], label: &str) -> Self {
        Self {
            tool,
            args: args.iter().map(|s| (*s).into()).collect(),
            directory: None,
            label: label.into(),
            timeout: 30,
            needs_secret: false,
            critical: false,
            secret: None,
        }
    }
    pub fn display(&self) -> String {
        if let Tool::External(path) = &self.tool {
            return format!(
                "{} [arguments omitted from logs]",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        let tool = match &self.tool {
            Tool::Adb => "adb".into(),
            Tool::Fastboot => "fastboot".into(),
            Tool::External(p) => p.display().to_string(),
        };
        let mut args = self.args.clone();
        if let Some(i) = args.iter().position(|s| s == "-s")
            && i + 1 < args.len()
        {
            args[i + 1] = "[selected device]".into();
        }
        if args.first().is_some_and(|s| s == "pair") && args.len() > 2 {
            args[2] = "[pairing code]".into();
        }
        format!(
            "{tool} {}",
            args.iter()
                .map(|a| format!("{a:?}"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
    pub fn redact_output(&self, text: &str) -> String {
        let mut result = text.to_owned();
        if let Some(secret) = &self.secret {
            result = result.replace(secret.value(), "[secret]");
        }
        if let Some(i) = self.args.iter().position(|v| v == "-s")
            && let Some(serial) = self.args.get(i + 1)
        {
            result = result.replace(serial, "[device]");
        }
        if self.args.first().is_some_and(|v| v == "pair")
            && let Some(code) = self.args.get(2)
        {
            result = result.replace(code, "[pairing code]");
        }
        if matches!(self.tool, Tool::External(_)) {
            for arg in &self.args {
                if arg.len() >= 4 && !arg.starts_with('-') {
                    result = result.replace(arg, "[argument]");
                }
            }
        }
        result
    }
}
#[derive(Clone, Debug)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}
impl Output {
    pub fn text(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
    pub fn require_success(self) -> Result<Self> {
        ensure!(
            self.code == 0,
            "Command failed ({}): {}",
            self.code,
            self.text()
        );
        Ok(self)
    }
}
pub trait Transport: Send + Sync {
    fn execute(
        &self,
        call: &Invocation,
        cancel: &AtomicBool,
        log: &(dyn Fn(String) + Send + Sync),
    ) -> Result<Output>;
}
#[derive(Clone)]
pub struct ProcessTransport {
    pub directory: PathBuf,
}
impl ProcessTransport {
    pub fn executable(&self, tool: &Tool) -> PathBuf {
        match tool {
            Tool::Adb => self
                .directory
                .join(if cfg!(windows) { "adb.exe" } else { "adb" }),
            Tool::Fastboot => self.directory.join(if cfg!(windows) {
                "fastboot.exe"
            } else {
                "fastboot"
            }),
            Tool::External(p) => p.clone(),
        }
    }
}
impl Transport for ProcessTransport {
    fn execute(
        &self,
        call: &Invocation,
        cancel: &AtomicBool,
        log: &(dyn Fn(String) + Send + Sync),
    ) -> Result<Output> {
        ensure!(!cancel.load(Ordering::Relaxed), "Operation cancelled");
        let program = self.executable(&call.tool);
        ensure!(
            program.is_file(),
            "Executable not found: {}. Select a platform-tools folder containing both adb and fastboot.",
            program.display()
        );
        log(format!("{}  ·  {}", call.label, call.display()));
        let mut command = Command::new(&program);
        ensure!(
            !call.needs_secret || call.secret.is_some(),
            "Fresh human secret input is required"
        );
        command
            .args(&call.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if call.secret.is_some() {
            command.stdin(Stdio::piped());
        }
        if let Some(dir) = &call.directory {
            command.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("Starting {}", program.display()))?;
        let stdout = child.stdout.take().unwrap();
        if let Some(secret) = &call.secret {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take()
                && let Err(error) = stdin
                    .write_all(secret.value().as_bytes())
                    .and_then(|_| stdin.write_all(b"\n"))
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let read = |reader: Box<dyn std::io::Read + Send>,
                    is_err: bool,
                    tx: std::sync::mpsc::Sender<(bool, String)>| {
            thread::spawn(move || {
                for line in BufReader::new(reader).lines() {
                    match line {
                        Ok(line) => {
                            if tx.send((is_err, line)).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
        };
        let a = read(Box::new(stdout), false, tx.clone());
        let b = read(Box::new(stderr), true, tx.clone());
        drop(tx);
        let start = Instant::now();
        let mut out = String::new();
        let mut err = String::new();
        let mut aborted = None;
        let mut timeout_reported = false;
        let code = loop {
            while let Ok((is_err, line)) = rx.try_recv() {
                let destination = if is_err { &mut err } else { &mut out };
                if destination.len() < 8 * 1024 * 1024 {
                    destination.push_str(&line);
                    destination.push('\n');
                }
                log(call.redact_output(&line));
            }
            let expired = start.elapsed() > Duration::from_secs(call.timeout.max(1));
            if expired && crate::operation::is_critical(call) && !timeout_reported {
                log("Critical command exceeded its estimate; waiting for command completion before the next safe checkpoint".into());
                timeout_reported = true;
            }
            if (cancel.load(Ordering::Relaxed) || expired) && !crate::operation::is_critical(call) {
                aborted = Some(if cancel.load(Ordering::Relaxed) {
                    "Operation cancelled; completed device writes cannot be undone"
                } else {
                    "Command timed out"
                });
                let _ = child.kill();
                break child.wait()?.code().unwrap_or(-1);
            }
            if let Some(status) = child.try_wait()? {
                break status.code().unwrap_or(-1);
            }
            thread::sleep(Duration::from_millis(25));
        };
        let _ = a.join();
        let _ = b.join();
        while let Ok((is_err, line)) = rx.try_recv() {
            let dest = if is_err { &mut err } else { &mut out };
            dest.push_str(&line);
            dest.push('\n');
        }
        if let Some(reason) = aborted {
            bail!("{reason}");
        }
        if call.secret.is_some()
            || matches!(call.tool, Tool::External(_))
            || call.args.first().is_some_and(|v| v == "pair")
        {
            out = call.redact_output(&out);
            err = call.redact_output(&err);
        }
        Ok(Output {
            code,
            stdout: out,
            stderr: err,
        })
    }
}
pub fn quiet(t: &dyn Transport, c: &Invocation) -> Result<Output> {
    t.execute(c, &AtomicBool::new(false), &|_| {})?
        .require_success()
}
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub fn validate_identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value != "."
            && value != ".."
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
        "Invalid identifier"
    );
    Ok(())
}
pub fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .context("Path is not valid Unicode")
}
pub fn cancellation() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
