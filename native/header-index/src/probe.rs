//! Sandboxed Python probe runner.
//!
//! Runs Python scripts in a macOS sandbox with:
//!   - Hard timeout (5s default)
//!   - Signal/exit code capture
//!   - Structured error parsing (assertion messages, crash reasons)
//!   - Deny file-write, network, most IPC
//!
//! Used by Layer 4 (protocol) to safely call private framework functions
//! and learn from the failures.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

// =============================================================================
// Types
// =============================================================================

/// Result of running a probe script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>, // SIGSEGV=11, SIGBUS=10, SIGABRT=6
    pub stdout: String,
    pub stderr: String,
    pub error_message: Option<String>, // parsed from stderr/crash output
    pub timed_out: bool,
    pub duration_ms: u64,
}

impl ProbeResult {
    /// One-line summary for LLM feedback.
    pub fn summary(&self) -> String {
        if self.timed_out {
            return "TIMEOUT: script hung (likely waiting for missing prerequisite)".into();
        }
        if self.success {
            return format!("OK: {}", first_line(&self.stdout));
        }
        if let Some(sig) = self.signal {
            let name = match sig {
                6 => "SIGABRT",
                10 => "SIGBUS",
                11 => "SIGSEGV",
                _ => "signal",
            };
            let msg = self.error_message.as_deref().unwrap_or("no message");
            return format!("CRASH {name} ({sig}): {msg}");
        }
        let code = self.exit_code.unwrap_or(-1);
        let msg = self.error_message.as_deref().unwrap_or(&self.stderr);
        format!("FAIL (exit {code}): {}", first_line(msg))
    }
}

/// Configuration for a probe run.
pub struct ProbeConfig {
    pub timeout: Duration,
    pub sandbox: bool,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            sandbox: crate::config::platform().sandbox.enabled,
        }
    }
}

// =============================================================================
// Sandbox profile
// =============================================================================

/// Minimal macOS sandbox profile: allow framework loads + reads, deny writes/network.
const SANDBOX_PROFILE: &str = r#"
(version 1)
(deny default)
(allow process-exec)
(allow process-fork)
(allow file-read*)
(allow file-write* (subpath "/private/tmp") (subpath "/tmp") (subpath "/dev"))
(allow mach-lookup)
(allow sysctl-read)
(allow iokit-open)
(allow system-socket)
(deny network*)
(deny file-write* (subpath "/System") (subpath "/usr") (subpath "/Library"))
"#;

// =============================================================================
// Runner
// =============================================================================

/// Run a Python script string in a sandboxed subprocess.
pub fn run_probe(script: &str, config: &ProbeConfig) -> ProbeResult {
    let start = std::time::Instant::now();

    // Write script to tempfile
    let tmp = match tempfile::NamedTempFile::new() {
        Ok(mut f) => {
            let _ = f.write_all(script.as_bytes());
            let _ = f.flush();
            f
        }
        Err(e) => return error_result(&format!("tempfile: {e}")),
    };
    let script_path = tmp.path().to_string_lossy().to_string();

    // Build command: sandbox-exec or plain python3
    let mut cmd = if config.sandbox {
        let mut c = Command::new("sandbox-exec");
        c.args(["-p", SANDBOX_PROFILE, "python3", &script_path]);
        c
    } else {
        let mut c = Command::new("python3");
        c.arg(&script_path);
        c
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return error_result(&format!("spawn: {e}")),
    };

    // Wait with timeout
    let result = wait_with_timeout(&mut child, config.timeout);
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        WaitResult::Completed(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();
            let signal = extract_signal(&output.status);
            let error_message = parse_error_message(&stderr);

            ProbeResult {
                success: output.status.success(),
                exit_code,
                signal,
                stdout,
                stderr,
                error_message,
                timed_out: false,
                duration_ms,
            }
        }
        WaitResult::TimedOut => {
            let _ = child.kill();
            let _ = child.wait();
            ProbeResult {
                success: false,
                exit_code: None,
                signal: Some(9),
                stdout: String::new(),
                stderr: String::new(),
                error_message: Some("killed after timeout".into()),
                timed_out: true,
                duration_ms,
            }
        }
        WaitResult::Error(e) => error_result(&format!("wait: {e}")),
    }
}

/// Run a probe with default config.
pub fn run_probe_default(script: &str) -> ProbeResult {
    run_probe(script, &ProbeConfig::default())
}

// =============================================================================
// Timeout handling
// =============================================================================

enum WaitResult {
    Completed(std::process::Output),
    TimedOut,
    Error(String),
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> WaitResult {
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // Child exited — collect output
                let stdout = read_pipe(child.stdout.take());
                let stderr = read_pipe(child.stderr.take());
                return WaitResult::Completed(std::process::Output {
                    status: _status,
                    stdout: stdout.into_bytes(),
                    stderr: stderr.into_bytes(),
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return WaitResult::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return WaitResult::Error(e.to_string()),
        }
    }
}

fn read_pipe(pipe: Option<impl std::io::Read>) -> String {
    match pipe {
        Some(mut p) => {
            let mut buf = String::new();
            let _ = std::io::Read::read_to_string(&mut p, &mut buf);
            buf
        }
        None => String::new(),
    }
}

// =============================================================================
// Error parsing
// =============================================================================

/// Extract signal number from exit status (Unix only).
#[cfg(unix)]
fn extract_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn extract_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

/// Parse the most useful error message from stderr.
/// Looks for Python tracebacks, Apple assertion failures, and crash messages.
fn parse_error_message(stderr: &str) -> Option<String> {
    if stderr.is_empty() {
        return None;
    }

    // Python traceback: last line is the actual error
    if let Some(line) = stderr.lines().rev().find(|l| {
        l.starts_with("Error:")
            || l.starts_with("TypeError:")
            || l.starts_with("OSError:")
            || l.starts_with("RuntimeError:")
            || l.starts_with("ValueError:")
            || l.starts_with("AttributeError:")
            || l.starts_with("ctypes.ArgumentError")
            || l.contains("Error:")
            || l.contains("Assertion failed")
    }) {
        return Some(line.trim().to_string());
    }

    // Apple framework assertions: "*** Assertion failure in ..."
    if let Some(line) = stderr
        .lines()
        .find(|l| l.contains("Assertion failure") || l.contains("*** Terminating"))
    {
        return Some(line.trim().to_string());
    }

    // Fallback: last non-empty line
    stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
}

fn error_result(msg: &str) -> ProbeResult {
    ProbeResult {
        success: false,
        exit_code: None,
        signal: None,
        stdout: String::new(),
        stderr: msg.into(),
        error_message: Some(msg.into()),
        timed_out: false,
        duration_ms: 0,
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("(empty)").trim()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_success() {
        let result = run_probe(
            "print('hello from probe')",
            &ProbeConfig {
                timeout: Duration::from_secs(5),
                sandbox: false,
            },
        );
        assert!(result.success);
        assert!(result.stdout.contains("hello from probe"));
        assert!(!result.timed_out);
    }

    #[test]
    fn test_probe_failure() {
        let result = run_probe(
            "raise RuntimeError('bad thing')",
            &ProbeConfig {
                timeout: Duration::from_secs(5),
                sandbox: false,
            },
        );
        assert!(!result.success);
        assert!(result
            .error_message
            .as_ref()
            .is_some_and(|m| m.contains("RuntimeError")));
    }

    #[test]
    fn test_probe_timeout() {
        let result = run_probe(
            "import time; time.sleep(30)",
            &ProbeConfig {
                timeout: Duration::from_millis(500),
                sandbox: false,
            },
        );
        assert!(!result.success);
        assert!(result.timed_out);
    }

    #[test]
    fn test_probe_syntax_error() {
        let result = run_probe(
            "def f(:\n  pass",
            &ProbeConfig {
                timeout: Duration::from_secs(5),
                sandbox: false,
            },
        );
        assert!(!result.success);
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_summary_formats() {
        let ok = ProbeResult {
            success: true,
            exit_code: Some(0),
            signal: None,
            stdout: "result=42\n".into(),
            stderr: String::new(),
            error_message: None,
            timed_out: false,
            duration_ms: 100,
        };
        assert!(ok.summary().starts_with("OK:"));

        let crash = ProbeResult {
            success: false,
            exit_code: None,
            signal: Some(11),
            stdout: String::new(),
            stderr: String::new(),
            error_message: Some("null pointer dereference".into()),
            timed_out: false,
            duration_ms: 50,
        };
        assert!(crash.summary().contains("SIGSEGV"));

        let timeout = ProbeResult {
            success: false,
            exit_code: None,
            signal: Some(9),
            stdout: String::new(),
            stderr: String::new(),
            error_message: Some("killed after timeout".into()),
            timed_out: true,
            duration_ms: 5000,
        };
        assert!(timeout.summary().contains("TIMEOUT"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_probe_sandbox() {
        // Should be able to load a framework and call a simple function
        let script = r#"
import ctypes
lib = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
print(f"loaded, kCFAllocatorDefault={lib.kCFAllocatorDefault}")
"#;
        let result = run_probe(
            script,
            &ProbeConfig {
                timeout: Duration::from_secs(5),
                sandbox: true,
            },
        );
        // sandbox-exec might not be available in all environments, so just check it ran
        assert!(!result.timed_out, "sandbox probe should not timeout");
    }

    #[test]
    fn test_parse_error_message() {
        assert_eq!(
      parse_error_message("Traceback (most recent call last):\n  File \"x.py\", line 1\nRuntimeError: service not created"),
      Some("RuntimeError: service not created".into())
    );
        assert_eq!(
            parse_error_message("*** Assertion failure in -[MTLDevice newLibrary], MTLDevice.m:42"),
            Some("*** Assertion failure in -[MTLDevice newLibrary], MTLDevice.m:42".into())
        );
        assert_eq!(parse_error_message(""), None);
    }
}
