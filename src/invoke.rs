//! How to start a harness binary.
//!
//! Windows `CreateProcess` cannot execute a `.cmd`/`.bat` shim, and most npm
//! installed harnesses *are* such shims. The fix avoids quoting entirely: the
//! child is `cmd.exe /d /s /c <stem> <args…>` where `<stem>` is a bare command
//! name (no quotes, no separators) resolved through `PATH`, which NextHarness points
//! at the binary's own directory. Every argument that follows is either a fixed
//! catalog token or the fixed prompt sentence — never user text — so `cmd.exe`
//! only ever re-parses text NextHarness wrote itself.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The program to spawn, and the arguments that must precede the harness's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// Executable to create, e.g. the harness itself or `cmd.exe`.
    pub program: PathBuf,
    /// Arguments inserted before the catalog's `run` template.
    pub prefix: Vec<String>,
}

/// True for the script shims Windows cannot execute directly.
pub fn is_windows_script(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
}

/// Work out how to start `binary` on this platform.
pub fn invocation(binary: &Path) -> Invocation {
    if cfg!(windows) && is_windows_script(binary) {
        let stem = binary
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();
        Invocation {
            program: PathBuf::from("cmd.exe"),
            prefix: vec!["/d".to_string(), "/s".to_string(), "/c".to_string(), stem],
        }
    } else {
        Invocation {
            program: binary.to_path_buf(),
            prefix: Vec::new(),
        }
    }
}

/// A `PATH` value that starts with the binary's directory, so a bare command
/// name still resolves inside the child process.
pub fn path_with(binary: &Path) -> Option<OsString> {
    let directory = binary.parent()?;
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut parts = vec![directory.to_path_buf()];
    parts.extend(std::env::split_paths(&existing));
    std::env::join_paths(parts).ok()
}

/// Run a short command and collect its output, killing it after `timeout`.
///
/// Used for version probes and `npm prefix -g`: both produce seconds of output
/// at most, so polling for exit before draining the pipes cannot fill them.
pub fn capture(
    invocation: &Invocation,
    args: &[String],
    timeout: Duration,
    path: Option<OsString>,
) -> Option<Output> {
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.prefix)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a probe must never flash a console window.
        command.creation_flags(0x0800_0000);
    }

    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + timeout;

    let finished = loop {
        match child.try_wait() {
            Ok(Some(_)) => break true,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                let _ = child.kill();
                break false;
            }
        }
    };

    if !finished {
        return None;
    }
    child.wait_with_output().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_are_detected_by_extension() {
        assert!(is_windows_script(Path::new("C:/x/opencode.cmd")));
        assert!(is_windows_script(Path::new("C:/x/opencode.BAT")));
        assert!(!is_windows_script(Path::new("C:/x/opencode.exe")));
        assert!(!is_windows_script(Path::new("C:/x/opencode")));
    }

    #[test]
    fn invocation_matches_the_platform() {
        let script = Path::new("C:/tools/opencode.cmd");
        let plan = invocation(script);
        if cfg!(windows) {
            assert_eq!(plan.program, PathBuf::from("cmd.exe"));
            assert_eq!(plan.prefix.last().map(String::as_str), Some("opencode"));
        } else {
            assert_eq!(plan.program, script);
            assert!(plan.prefix.is_empty());
        }

        let native = invocation(Path::new("/usr/bin/opencode"));
        assert!(native.prefix.is_empty());
    }
}
