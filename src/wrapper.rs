//! Steam launch-wrapper mode. Placed in BF3's Steam launch options as
//! `bf3-linux-setup --steam-launch -- %command%`. Before the game starts it
//! refreshes the PunkBuster security file (so PunkBuster self-updates from the
//! PB servers on launch), then replaces itself with the real game command.
//!
//! Robustness rule: nothing here may ever prevent the game from launching. Any
//! network or filesystem failure is logged to stderr and ignored.

use crate::i18n::{t, tf, Key};
use crate::punkbuster;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Refresh PunkBuster if stale, then exec the real game command.
///
/// `argv` is everything Steam expanded from `%command%` (program + args).
/// Returns an exit code only on failure to launch; on success it never returns
/// (the process image is replaced).
pub fn run(argv: &[String]) -> i32 {
    refresh_punkbuster();

    let (program, rest) = match argv.split_first() {
        Some(pair) => pair,
        None => {
            eprintln!("{}", t(Key::WrapNoGameCmd));
            return 2;
        }
    };
    // Replace this process with the game. Only returns if exec itself fails.
    let err = Command::new(program).args(rest).exec();
    eprintln!("{}", tf(Key::WrapLaunchFailedTmpl, &[program, &err.to_string()]));
    127
}

/// Best-effort PunkBuster refresh. Never panics, never blocks the launch.
fn refresh_punkbuster() {
    let bf3 = match punkbuster::find_bf3() {
        Some(b) => b,
        None => {
            eprintln!("{}", t(Key::WrapBf3NotFound));
            return;
        }
    };
    let body = match punkbuster::fetch_pbsec() {
        Some(b) => b,
        None => {
            eprintln!("{}", t(Key::WrapNoEvenBalance));
            return;
        }
    };
    let online = punkbuster::parse_pbsec_client_version(&body);
    let installed = punkbuster::installed_client_version(&bf3).map(|(v, _)| v);

    let stale = match (online, installed) {
        (Some(o), Some(i)) => o > i,
        (Some(_), None) => true, // client unknown → refresh to be safe
        _ => false,              // no online info → nothing to do
    };

    if stale {
        match punkbuster::refresh_pbsec_file(&bf3, &body) {
            Ok(path) => eprintln!(
                "{}",
                tf(
                    Key::WrapRefreshedTmpl,
                    &[
                        &path.display().to_string(),
                        &online.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                    ],
                )
            ),
            Err(e) => eprintln!("{}", tf(Key::WrapWriteFailedTmpl, &[&e.to_string()])),
        }
    } else {
        eprintln!("{}", t(Key::WrapAlreadyCurrent));
    }
}
