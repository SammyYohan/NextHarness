//! The command line: the only way a human talks to NextHarness.
//!
//! Every command is a thin shell around a library function. The rules that
//! matter — never spending without a click, never driving an interactive
//! harness — live in [`crate::gatekeeper`] and [`crate::orchestrator`], not here.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::detector::{Detection, Detector};
use crate::doctor;
use crate::error::Result;
use crate::gatekeeper::{SystemUi, Ui};
use crate::git;
use crate::handoff;
use crate::i18n;
use crate::orchestrator::{self, Conclusion, Session, Settings};
use crate::paths::Paths;
use crate::prefs::Preferences;
use crate::state::State;
use crate::timefmt::{format_utc, now_epoch};
use crate::{cache, VERSION};

#[derive(Debug, Parser)]
#[command(
    name = "next-harness",
    version = VERSION,
    about = "One coding task, relayed across the coding harnesses installed on this machine",
    disable_help_subcommand = true
)]
struct Cli {
    /// Interface language: fr or en
    #[arg(long, global = true, value_name = "LANG")]
    lang: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Detect installed harnesses and write the default configuration
    Init,
    /// Run a task across the harnesses, in catalog order
    Run {
        /// What to do
        task: String,
        /// Maximum number of harnesses to try
        #[arg(long, value_name = "N")]
        max_relays: Option<usize>,
        /// Also prepare human-driven harnesses
        #[arg(long)]
        manual: bool,
    },
    /// Continue the task already recorded in .next-harness/TASK.md
    Resume {
        /// Maximum number of harnesses to try
        #[arg(long, value_name = "N")]
        max_relays: Option<usize>,
        /// Also prepare human-driven harnesses
        #[arg(long)]
        manual: bool,
    },
    /// Show which harnesses are paused and until when
    Status,
    /// Check this machine and report what was found
    Doctor {
        /// Ignore the detection cache
        #[arg(long)]
        refresh: bool,
        /// Print a Markdown report that can be pasted into an issue
        #[arg(long)]
        report: bool,
    },
    /// Show where the configuration lives
    Config,
}

/// Parse and run, returning the process exit code.
pub fn run(args: Vec<String>) -> i32 {
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return error.exit_code();
        }
    };

    i18n::init(cli.lang.as_deref());
    match dispatch(cli.command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn dispatch(command: Command) -> Result<i32> {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = Paths::resolve(&root)?;
    let config = Config::load(&paths.config_dir)?;
    let prefs = Preferences::load(&paths.config_file);

    match command {
        Command::Init => init(&paths, &config),
        Command::Run {
            task,
            max_relays,
            manual,
        } => start(&paths, &config, &prefs, &root, &task, max_relays, manual),
        Command::Resume { max_relays, manual } => {
            resume(&paths, &config, &prefs, &root, max_relays, manual)
        }
        Command::Status => status(&paths, &config),
        Command::Doctor { refresh, report } => check(&paths, &config, refresh, report),
        Command::Config => show_config(&paths, &config, &prefs),
    }
}

/// Detect harnesses, preferring the cache unless a refresh was asked for.
fn detect(paths: &Paths, config: &Config, refresh: bool) -> Vec<Detection> {
    let expected = config.catalog.harnesses.len();
    if !refresh {
        if let Some(entries) = cache::load(&paths.cache_file, expected) {
            return entries;
        }
    }
    let detections = Detector::new(config).detect_all(config);
    cache::store(&paths.cache_file, &detections);
    detections
}

fn init(paths: &Paths, config: &Config) -> Result<i32> {
    println!("{}", i18n::tr("cli.init.title"));
    println!("{}", i18n::tr("cli.init.detecting"));

    let detections = detect(paths, config, true);
    let usable: Vec<&Detection> = detections
        .iter()
        .filter(|entry| entry.status.is_usable())
        .collect();

    if usable.is_empty() {
        println!("{}", i18n::tr("cli.init.none"));
        return Ok(1);
    }

    let order = usable
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<String>>()
        .join(", ");
    println!("{}", i18n::tr_args("cli.init.order", &[("list", order)]));

    for entry in &usable {
        println!(
            "{}",
            i18n::tr_args(
                "cli.init.status_line",
                &[
                    ("id", entry.id.clone()),
                    ("status", i18n::tr(entry.status.label_key()).to_string()),
                    ("tier", entry.tier.label().to_string()),
                ],
            )
        );
        if entry.tier.requires_consent() {
            // The default answer is the safe one, so it is stated, not asked.
            println!(
                "{}",
                i18n::tr_args("cli.init.unknown_tier", &[("name", entry.name.clone())])
            );
        }
    }

    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if git::is_repo(&root) {
        println!("{}", i18n::tr("cli.init.branch"));
    } else {
        println!("{}", i18n::tr("cli.init.not_repo"));
    }

    let prefs = Preferences::load(&paths.config_file);
    prefs.save(&paths.config_file)?;
    println!(
        "{}",
        i18n::tr_args(
            "cli.init.saved",
            &[("path", paths.config_file.display().to_string())]
        )
    );
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
fn start(
    paths: &Paths,
    config: &Config,
    prefs: &Preferences,
    root: &std::path::Path,
    task: &str,
    max_relays: Option<usize>,
    manual: bool,
) -> Result<i32> {
    if !git::is_repo(root) {
        println!("{}", i18n::tr("cli.run.not_repo"));
    }

    let settings = Settings {
        max_relays: max_relays.unwrap_or(prefs.max_relays),
        manual: manual || prefs.manual,
    };
    relay(paths, config, root, task, settings)
}

fn resume(
    paths: &Paths,
    config: &Config,
    prefs: &Preferences,
    root: &std::path::Path,
    max_relays: Option<usize>,
    manual: bool,
) -> Result<i32> {
    if !paths.task_file.is_file() {
        println!("{}", i18n::tr("handoff.missing"));
        return Ok(1);
    }
    let task = handoff::read_task(paths)?;
    if !git::is_repo(root) {
        println!("{}", i18n::tr("cli.run.not_repo"));
    }

    let settings = Settings {
        max_relays: max_relays.unwrap_or(prefs.max_relays),
        manual: manual || prefs.manual,
    };
    relay(paths, config, root, &task, settings)
}

fn relay(
    paths: &Paths,
    config: &Config,
    root: &std::path::Path,
    task: &str,
    settings: Settings,
) -> Result<i32> {
    let detections = detect(paths, config, false);
    let mut state = State::load(&paths.state_file);
    let ui = SystemUi::new();

    let session = Session {
        config,
        paths,
        detections: &detections,
        root,
        task,
        ui: &ui as &dyn Ui,
        settings,
    };
    let report = orchestrator::run(&session, &mut state)?;

    Ok(match report.conclusion {
        Conclusion::Finished | Conclusion::Manual => 0,
        Conclusion::Exhausted | Conclusion::Stopped => 1,
    })
}

fn status(paths: &Paths, config: &Config) -> Result<i32> {
    println!("{}", i18n::tr("cli.status.title"));
    let now = now_epoch();
    let mut state = State::load(&paths.state_file);
    state.prune(now);

    if state.pauses.is_empty() {
        println!("{}", i18n::tr("cli.status.empty"));
        println!("{}", i18n::tr("cli.status.hint"));
        return Ok(0);
    }

    for (id, pause) in &state.pauses {
        let name = config
            .harness(id)
            .map(|harness| harness.name.clone())
            .unwrap_or_else(|| id.clone());
        println!(
            "{}",
            i18n::tr_args(
                "cli.status.paused",
                &[
                    ("name", name),
                    ("when", format_utc(pause.until)),
                    ("kind", pause.kind.clone()),
                ],
            )
        );
    }
    Ok(0)
}

fn check(paths: &Paths, config: &Config, refresh: bool, report: bool) -> Result<i32> {
    let detections = detect(paths, config, refresh);

    if report {
        // Markdown on stdout: the whole point is that it can be pasted as is.
        print!("{}", doctor::markdown(config, &detections, paths));
        return Ok(0);
    }

    println!("{}", i18n::tr("cli.doctor.title"));
    for line in doctor::lines(config, &detections, paths) {
        println!("{line}");
    }
    if config.warnings.is_empty() {
        println!("{}", i18n::tr("cli.doctor.ok"));
    }
    Ok(0)
}

fn show_config(paths: &Paths, config: &Config, prefs: &Preferences) -> Result<i32> {
    println!("{}", i18n::tr("cli.config.title"));
    println!(
        "{}",
        i18n::tr_args(
            "cli.config.path",
            &[("path", paths.config_dir.display().to_string())]
        )
    );
    if config.override_files.is_empty() {
        println!("{}", i18n::tr("cli.config.none"));
    } else {
        for file in &config.override_files {
            println!(
                "{}",
                i18n::tr_args(
                    "cli.config.override",
                    &[("path", file.display().to_string())]
                )
            );
        }
    }
    println!(
        "{}",
        i18n::tr_args(
            "cli.config.state",
            &[("path", paths.state_file.display().to_string())]
        )
    );
    println!(
        "{}",
        i18n::tr_args(
            "cli.config.settings",
            &[
                ("count", prefs.max_relays.to_string()),
                ("manual", prefs.manual.to_string()),
                ("lang", rust_i18n::locale().to_string()),
            ],
        )
    );
    Ok(0)
}
