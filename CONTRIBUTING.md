# Contributing

Thanks for helping. NextHarness is deliberately small: a catalogue, a detector, a
runner, a quota rule and a handoff loop. Contributions that keep it that way are
the easiest to merge.

## Build

Rust 1.85 or newer.

```sh
cargo build            # debug
cargo build --release  # what users run
```

Linux needs no extra packages to build. At runtime the pay prompt is drawn by
zenity or kdialog when one of them is installed; without either, NextHarness
asks at the terminal or waits — it never spends by itself.

## Before opening a pull request

All three must pass, and CI runs them on Windows, Ubuntu and macOS:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Two rules that are not negotiable:

- **No `unwrap()` outside tests.** Errors are values (`crate::Error`). The only
  `expect()` calls left are on regular expressions compiled from string literals
  at start-up.
- **No `TODO`.** If something is not finished, it is not merged.

## Adding or fixing a harness

Everything the program knows about a tool comes from `profiles/builtin.toml`.
That file is compiled into the binary, so a change there needs a rebuild and a
test — nothing else.

An entry must use **only what the vendor documents**: the real binary name, the
real flags. Never invent a flag, and never add a path that is not documented.
If you guessed, say so in `notes` and set `verified = "unverified"`.

```toml
[[harness]]
id = "example"                 # stable, used by overrides and by state.json
name = "Example CLI"
mode = "headless"              # headless | interactive | gui
binaries = ["example"]         # tried in order, on PATH
companion_binaries = []        # every one of these must also exist
version_args = ["--version"]   # must exit 0 and print something
run = ["-p", "{prompt}"]       # must contain {prompt}
run_resume = []
unattended_args = []           # only if the vendor documents them
unattended_required = false
platforms = ["linux", "macos", "windows"]
default_tier = "free"          # free | paid | unknown
verified = "unverified"        # official | third_party | unverified
quota_ignores_exit_code = false
```

- `mode = "interactive"` means a human drives it: NextHarness prepares the prompt and
  never automates the interface.
- `mode = "gui"` means it is only detected by name (`gui_keywords`).
- `default_tier = "unknown"` is treated as **paid**: the user is asked before it
  runs. Do not use `unknown` to mean "probably fine".

Add a test in `tests/config.rs` or `tests/detector.rs` when the entry exercises a
new rule.

### Quota patterns

Patterns live in `[detection_generic]` (shared) or in a harness's
`[harness.detection]`. A pattern is one signal among two — see
[README](README.md#why-a-harness-is-paused). Keep them narrow: a false positive
pauses a working harness, which is worse than a missed detection.

## The test suite

- `src/quota.rs` (unit tests) — the two-signal rule, including the false
  positive that must stay inert.
- `tests/runner.rs` — the pseudo-terminal, timeouts, killing.
- `tests/handoff.rs` — the whole handoff loop, using the repository's own fake harness.
- `tests/cli.rs` — the binary as a user meets it.
- `src/bin/fake_harness.rs` — the fixture all of the above use. It is never
  shipped: the release workflow builds `--bin next-harness` only.

Tests must never need a network, an account, real quota, or a dialog. If a test
opens a window, it is a bug.

## Reporting a machine

The catalogue's `verified` column improves with real reports. Run:

```sh
next-harness doctor --report
```

and paste the result into a [test report](.github/ISSUE_TEMPLATE/test-report.md).
The report contains no account name, no path with your user name and no
credential — please check it before posting anyway.

## Style

Comments explain *why*, not *what*. Every public item carries a doc comment.
User-facing text goes through `locales/`. The test suite keeps all eight
locales identical key for key and placeholder for placeholder.
