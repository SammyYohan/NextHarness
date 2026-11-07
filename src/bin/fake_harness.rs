//! A fake harness, shipped with the repository so the test suite can exercise
//! the relay without network access, accounts or quota.
//!
//! It is only ever built as a test fixture and is excluded from release
//! artefacts.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "success".to_string());

    match mode.as_str() {
        "success" => {
            println!("working");
            println!("done");
            ExitCode::SUCCESS
        }
        "quota-exit" => {
            println!("rate limit reached");
            println!("quota exceeded for this period");
            ExitCode::from(1)
        }
        "quota-zero" => {
            println!("usage limit exceeded");
            ExitCode::SUCCESS
        }
        "word-in-success" => {
            println!("I renamed the quota struct and the rate limit helper.");
            for line in 0..60 {
                println!("line {line} of a long, successful build log");
            }
            ExitCode::SUCCESS
        }
        "quota-leak" => {
            // A usage-limit message that also echoes something private, to prove
            // the handoff masks it.
            println!("rate limit reached for key sk-abcdefghijklmnop");
            ExitCode::from(1)
        }
        "auth" => {
            eprintln!("invalid api key");
            ExitCode::from(2)
        }
        "silent" => {
            // Nothing at all: the runner must notice the missing startup output.
            std::thread::sleep(std::time::Duration::from_secs(3_600));
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("fake-harness: unknown mode {other}");
            ExitCode::from(64)
        }
    }
}
