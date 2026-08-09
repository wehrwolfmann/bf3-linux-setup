//! Battlefield 3 Linux setup — Rust rewrite.
//!
//! Modes:
//!   (no args)                  → GUI setup window (default)
//!   --steam-launch -- CMD...   → headless launch wrapper (goes in Steam)
//!   --setup                    → run the setup in the terminal
//!   --ua-off                   → remove the Firefox OS spoof
//!   --help                     → usage

mod actions;
mod consts;
mod firefox;
mod gui;
mod i18n;
mod launcher;
mod punkbuster;
mod steam_launch;
mod uninstall;
mod wrapper;

use actions::{Level, Log};
use i18n::{t, tf, Key};
use std::io::IsTerminal;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Launch-wrapper mode: refresh PunkBuster, then exec the real game command.
    if let Some(pos) = args.iter().position(|a| a == "--steam-launch") {
        let after = &args[pos + 1..];
        let cmd = match after.iter().position(|a| a == "--") {
            Some(sep) => &after[sep + 1..],
            None => after,
        };
        return ExitCode::from(wrapper::run(cmd) as u8);
    }

    match args.first().map(String::as_str) {
        None => {
            // Default: GUI.
            if let Err(e) = gui::run() {
                eprintln!("{}", tf(Key::GuiOpenFailedTmpl, &[&e.to_string()]));
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") => {
            print_usage();
            ExitCode::SUCCESS
        }
        Some("--setup") => {
            let mut log = Log::new();
            log.header(t(Key::CliSetupHeader));
            log.blank();
            actions::run_setup(&mut log);
            render_terminal(&log);
            ExitCode::SUCCESS
        }
        Some("--ua-off") => {
            let mut log = Log::new();
            actions::do_ua_off(&mut log);
            render_terminal(&log);
            ExitCode::SUCCESS
        }
        // Same as the GUI's "Add to app menu" button, for people who install
        // the released binary from a terminal and never open the window.
        Some("--install-launcher") => {
            let home = launcher::home_dir();
            let exe = std::path::PathBuf::from(launcher::current_exe_string());
            let mut log = Log::new();
            match launcher::install(&home, &exe) {
                Ok(paths) => {
                    log.ok(tf(
                        Key::LauncherInstalledTmpl,
                        &[&paths.desktop.display().to_string()],
                    ));
                    render_terminal(&log);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    log.warn(tf(Key::LauncherFailedTmpl, &[&e]));
                    render_terminal(&log);
                    ExitCode::from(1)
                }
            }
        }
        Some(other) => {
            eprintln!("{}", tf(Key::UnknownOptionTmpl, &[other]));
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    println!("{}", t(Key::UsageBlock));
}

/// Print a [`Log`] to the terminal, colourised when stdout is a TTY.
fn render_terminal(log: &Log) {
    let tty = std::io::stdout().is_terminal();
    for line in &log.lines {
        let (code, prefix) = match line.level {
            Level::Header => ("34", ""),
            Level::Ok => ("32", "✔ "),
            Level::Warn => ("33", "! "),
            Level::Info => ("2", "· "),
            Level::Plain => ("", ""),
        };
        let indent = matches!(line.level, Level::Ok | Level::Warn | Level::Info);
        let body = format!("{}{}{}", if indent { "  " } else { "" }, prefix, line.text);
        if tty && !code.is_empty() {
            println!("\x1b[{code}m{body}\x1b[0m");
        } else {
            println!("{body}");
        }
    }
}
