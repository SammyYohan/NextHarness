//! Starting one harness under a pseudo-terminal.
//!
//! A pseudo-terminal is the default because several harnesses print nothing at
//! all when stdout is not a terminal. Output is captured raw and ANSI codes are
//! stripped before anything is classified.
//!
//! Two details matter on Windows. Closing the slave side right after the spawn
//! tears the console down and kills the child, and the console host waits for a
//! cursor-position report before it starts forwarding the child's output. NextHarness
//! is the terminal here, so it answers that query itself (see
//! [`answer_cursor_query`]).

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::error::{Error, Result};
use crate::invoke::{invocation, path_with};
use crate::util::strip_ansi;

/// Default inactivity budget: no output for this long means the harness hung.
pub const DEFAULT_INACTIVITY: Duration = Duration::from_secs(120);

/// Default hard ceiling for one attempt.
pub const DEFAULT_ABSOLUTE: Duration = Duration::from_secs(30 * 60);

/// Timing budgets for one attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    /// How long NextHarness waits for new output before calling it a hang.
    pub inactivity: Duration,
    /// Hard ceiling for the whole attempt.
    pub absolute: Duration,
    /// Extra time allowed at startup, from the catalog's `startup_grace_s`.
    pub startup_grace: Duration,
    /// Terminal height reported to the harness.
    pub rows: u16,
    /// Terminal width reported to the harness.
    pub cols: u16,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            inactivity: DEFAULT_INACTIVITY,
            absolute: DEFAULT_ABSOLUTE,
            startup_grace: Duration::ZERO,
            rows: 40,
            cols: 120,
        }
    }
}

impl Params {
    /// `startup_grace_s` from the catalog, in place of the plain inactivity
    /// budget while nothing has been printed yet.
    pub fn with_startup_grace(mut self, grace_seconds: Option<u64>) -> Self {
        self.startup_grace = grace_seconds.map_or(Duration::ZERO, Duration::from_secs);
        self
    }
}

/// What one attempt produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Process exit code, when the platform reported one.
    pub exit_code: Option<u32>,
    /// The whole transcript, ANSI stripped.
    pub output: String,
    /// The absolute ceiling was hit.
    pub timed_out: bool,
    /// No output at all before the inactivity budget expired.
    pub stalled: bool,
    /// Wall-clock duration.
    pub duration: Duration,
}

impl Outcome {
    /// True when the harness reported success in the ordinary way.
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && !self.stalled
    }

    /// Number of characters produced.
    pub fn length(&self) -> usize {
        self.output.chars().count()
    }
}

/// Run `binary` with `args` inside `cwd` under a pseudo-terminal.
pub fn run(binary: &Path, args: &[String], cwd: &Path, params: &Params) -> Result<Outcome> {
    let plan = invocation(binary);
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: params.rows,
            cols: params.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| Error::msg(format!("cannot allocate a pseudo-terminal: {error}")))?;

    let mut command = CommandBuilder::new(&plan.program);
    for arg in &plan.prefix {
        command.arg(arg);
    }
    for arg in args {
        command.arg(arg);
    }
    command.cwd(cwd);
    if let Some(path) = path_with(binary) {
        command.env("PATH", path);
    }
    command.env("NO_COLOR", "1");
    command.env("TERM", "dumb");

    let started = Instant::now();
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| Error::msg(format!("cannot start {}: {error}", binary.display())))?;
    // The slave stays alive for the whole run: on Windows it owns the console,
    // so closing it early kills the child.

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| Error::msg(format!("cannot read the pseudo-terminal: {error}")))?;
    let mut writer = pair
        .master
        .take_writer()
        .map_err(|error| Error::msg(format!("cannot write to the pseudo-terminal: {error}")))?;

    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    if sender.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut collected: Vec<u8> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut timed_out = false;
    let mut stalled = false;
    let mut spoke = false;
    let mut last_word = started;

    loop {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                answer_cursor_query(&mut writer, &chunk, &mut pending);
                if has_words(&chunk) {
                    spoke = true;
                    last_word = Instant::now();
                }
                collected.extend_from_slice(&chunk);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // The console can stay open after the child exits, so the end of the
        // run is detected on the process, not on end-of-file.
        if matches!(child.try_wait(), Ok(Some(_))) {
            drain(&receiver, &mut writer, &mut pending, &mut collected);
            break;
        }

        let elapsed = started.elapsed();
        if elapsed > params.absolute {
            timed_out = true;
            break;
        }
        // Silence before the first real output is judged against the catalog's
        // startup grace; afterwards the harness gets the ordinary inactivity
        // budget. Console escape chatter does not count as speaking.
        let budget = if spoke || params.startup_grace == Duration::ZERO {
            params.inactivity
        } else {
            params.startup_grace
        };
        if last_word.elapsed() > budget {
            stalled = true;
            break;
        }
    }

    if timed_out || stalled {
        terminate(&mut child);
    }

    let exit_code = child.wait().ok().map(|status| status.exit_code());
    let duration = started.elapsed();

    // The reader stops on its own when the console closes. It is not joined: a
    // console host that outlives its child would otherwise hang the runner.
    drop(reader_thread);
    drop(writer);
    drop(pair.master);

    Ok(Outcome {
        exit_code,
        output: strip_ansi(&String::from_utf8_lossy(&collected)),
        timed_out,
        stalled,
        duration,
    })
}

/// Collect whatever is still buffered right after the child exited.
fn drain<W: std::io::Write>(
    receiver: &mpsc::Receiver<Vec<u8>>,
    writer: &mut W,
    pending: &mut Vec<u8>,
    collected: &mut Vec<u8>,
) {
    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(chunk) => {
                answer_cursor_query(writer, &chunk, pending);
                collected.extend_from_slice(&chunk);
            }
            Err(_) => break,
        }
    }
}

/// A chunk counts as output only when it carries something other than terminal
/// escape sequences and whitespace.
fn has_words(chunk: &[u8]) -> bool {
    let text = String::from_utf8_lossy(chunk);
    !strip_ansi(&text).trim().is_empty()
}

/// Answer the console host's cursor-position report (`ESC[6n`).
///
/// A terminal emulator answers with `ESC[<row>;<col>R`; without that answer the
/// Windows console host stops forwarding the child's output, so the harness
/// looks silent and the orchestrator times it out. The report may be split across two
/// reads, hence `pending`.
fn answer_cursor_query<W: std::io::Write>(writer: &mut W, chunk: &[u8], pending: &mut Vec<u8>) {
    const QUERY: &[u8] = b"\x1b[6n";
    let mut window = std::mem::take(pending);
    window.extend_from_slice(chunk);

    if window.windows(QUERY.len()).any(|part| part == QUERY) {
        let _ = writer.write_all(b"\x1b[1;1R");
        let _ = writer.flush();
    }

    let keep = QUERY.len() - 1;
    *pending = window[window.len().saturating_sub(keep)..].to_vec();
}

/// Kill the child and, on Windows, its whole tree: an agent left running after a
/// timeout would keep spending quota.
fn terminate(child: &mut Box<dyn portable_pty::Child + Send + Sync>) {
    let pid = child.process_id();
    let _ = child.kill();

    #[cfg(windows)]
    if let Some(pid) = pid {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(not(windows))]
    let _ = pid;
}
