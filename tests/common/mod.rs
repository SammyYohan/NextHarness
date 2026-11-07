//! Shared test scaffolding: a temporary directory that cleans itself up.
//!
//! `tempfile` is deliberately not a dependency; a PID + counter suffix is enough
//! for tests that never leave the temp directory.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A directory under the system temp dir, removed on drop.
pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    /// Create a fresh directory named after `tag`.
    pub fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("next-harness-{}-{tag}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create the temporary directory");
        Self { path }
    }

    /// The directory itself.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A path inside the directory.
    pub fn join(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }

    /// Write a file, creating parent directories.
    pub fn write(&self, relative: &str, body: &str) -> PathBuf {
        let file = self.join(relative);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("create the parent directory");
        }
        std::fs::write(&file, body).expect("write the test file");
        file
    }
}

/// Write a fake harness: a `.cmd` shim on Windows, a shell script elsewhere.
///
/// On Windows the shim cannot be executed directly, which is exactly the
/// situation Orion has to handle for every npm-installed harness.
pub fn write_script(dir: &Path, name: &str, lines: &[&str], exit_code: i32) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create the script directory");
    if cfg!(windows) {
        let file = dir.join(format!("{name}.cmd"));
        let mut body = String::from("@echo off\r\n");
        for line in lines {
            body.push_str(&format!("echo {line}\r\n"));
        }
        body.push_str(&format!("exit /b {exit_code}\r\n"));
        std::fs::write(&file, body).expect("write the shim");
        file
    } else {
        let file = dir.join(name);
        let mut body = String::from("#!/bin/sh\n");
        for line in lines {
            body.push_str(&format!("echo \"{line}\"\n"));
        }
        body.push_str(&format!("exit {exit_code}\n"));
        std::fs::write(&file, body).expect("write the script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&file)
                .expect("script metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&file, permissions).expect("make the script executable");
        }
        file
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
