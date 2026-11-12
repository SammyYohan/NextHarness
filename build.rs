//! Records the compiler version into the binary, so a `doctor --report` states
//! which toolchain produced it: a bug report is only reproducible against the
//! same rustc. The package's `rust-version` field is only the minimum and says
//! nothing about the compiler that was actually used.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    if let Ok(output) = std::process::Command::new(rustc).arg("--version").output() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            println!("cargo:rustc-env=NEXTHARNESS_RUSTC={text}");
        }
    }
}
