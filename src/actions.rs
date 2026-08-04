//! High-level operations shared by the CLI and the GUI. Each returns a [`Log`]
//! of tagged lines plus an overall [`Status`], mirroring the reference Python's
//! Reporter-driven flow so terminal and GUI stay in lock-step.

use crate::consts::{windows_ua, BF3_APPID};
use crate::firefox::VersionSource;
use crate::i18n::{t, tf, Key};
use crate::{firefox, punkbuster};
use std::path::Path;

/// Severity of a single log line (drives colour in both terminal and GUI).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Header,
    Ok,
    Warn,
    Info,
    Plain,
}

/// Overall result of a step, for the GUI's green/red status dots.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
}

/// One rendered log line.
pub struct Line {
    pub level: Level,
    pub text: String,
}

/// An ordered collection of log lines produced by an action.
#[derive(Default)]
pub struct Log {
    pub lines: Vec<Line>,
}

impl Log {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }
    fn push(&mut self, level: Level, text: impl Into<String>) {
        self.lines.push(Line {
            level,
            text: text.into(),
        });
    }
    pub fn header(&mut self, t: impl Into<String>) {
        self.push(Level::Header, t);
    }
    pub fn ok(&mut self, t: impl Into<String>) {
        self.push(Level::Ok, t);
    }
    pub fn warn(&mut self, t: impl Into<String>) {
        self.push(Level::Warn, t);
    }
    pub fn info(&mut self, t: impl Into<String>) {
        self.push(Level::Info, t);
    }
    pub fn plain(&mut self, t: impl Into<String>) {
        self.push(Level::Plain, t);
    }
    pub fn blank(&mut self) {
        self.push(Level::Plain, "");
    }
}

/// Step 1: write the four OS-spoof prefs into the default Firefox profile(s).
pub fn do_ua(log: &mut Log) -> Status {
    log.header(t(Key::HdrUa));
    let bases = firefox::firefox_bases();
    if bases.is_empty() {
        log.warn(t(Key::NoFirefoxProfile));
        return Status::Warn;
    }
    // The signature has to name a browser version the wider web still accepts:
    // an outdated one is answered with a bare 403 on more and more sites, so a
    // frozen number would fix Battlelog and quietly break everything else.
    log.info(t(Key::CheckingChromeVersion));
    let (chrome_version, source) = firefox::chrome_ua_version();
    match source {
        VersionSource::Online => log.ok(tf(Key::ChromeVersionOnlineTmpl, &[&chrome_version])),
        VersionSource::Fallback => log.warn(tf(Key::ChromeVersionFallbackTmpl, &[&chrome_version])),
    }
    let ua = windows_ua(&chrome_version);

    let mut touched = 0;
    for base in &bases {
        for prof in firefox::default_profiles(base) {
            firefox::set_ua(&prof, &ua);
            let name = prof.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            log.ok(tf(Key::OsSpoofWrittenTmpl, &[&name]));
            touched += 1;
        }
    }
    if touched == 0 {
        log.warn(t(Key::NoDefaultProfile));
        return Status::Warn;
    }
    if firefox::firefox_running() {
        log.warn(t(Key::FirefoxRunningRestart));
    } else {
        log.info(t(Key::LaunchFirefoxHint));
    }
    Status::Ok
}

/// Undo step 1: remove our marker-tagged prefs from every profile.
pub fn do_ua_off(log: &mut Log) -> Status {
    log.header(t(Key::HdrRemovingUa));
    let mut removed = 0;
    for base in firefox::firefox_bases() {
        for prof in firefox::default_profiles(&base) {
            if firefox::unset_ua(&prof) {
                let name = prof.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                log.ok(tf(Key::UaRemovedTmpl, &[&name]));
                removed += 1;
            }
        }
    }
    if removed == 0 {
        log.info(t(Key::NoChangesToRemove));
    } else if firefox::firefox_running() {
        log.warn(t(Key::RestartFirefoxNormalUa));
    }
    Status::Ok
}

/// Step 2: report installed PunkBuster client version and compare with the
/// current Even Balance version.
pub fn do_punkbuster(log: &mut Log) -> Status {
    log.blank();
    log.header(t(Key::HdrPunkbuster));
    let bf3 = match punkbuster::find_bf3() {
        Some(b) => b,
        None => {
            log.warn(t(Key::Bf3NotFound));
            return Status::Warn;
        }
    };
    log.info(tf(Key::GameTmpl, &[&bf3.display().to_string()]));
    punkbuster_status(&bf3, log)
}

fn punkbuster_status(bf3: &Path, log: &mut Log) -> Status {
    let pb_dll_dir = bf3.join("pb/dll");
    if !pb_dll_dir.is_dir() {
        log.warn(t(Key::PbFolderMissing));
        return Status::Warn;
    }
    let (installed_ver, newest_file) = match punkbuster::installed_client_version(bf3) {
        Some(v) => v,
        None => {
            log.warn(t(Key::NoPbClientDlls));
            return Status::Warn;
        }
    };
    log.info(tf(Key::InstalledVersionTmpl, &[&installed_ver.to_string()]));

    log.info(t(Key::CheckingOnline));
    let online = punkbuster::punkbuster_latest_online();
    match online {
        None => {
            log.warn(t(Key::OnlineUnreadable));
        }
        Some(o) if o > installed_ver => {
            log.warn(tf(
                Key::NewerAvailableTmpl,
                &[&o.to_string(), &installed_ver.to_string()],
            ));
            pb_update_help(log);
            return Status::Warn;
        }
        Some(o) => {
            log.ok(tf(Key::UpToDateTmpl, &[&o.to_string()]));
        }
    }

    if punkbuster::pbcl_matches_newest(bf3, &newest_file) {
        log.ok(t(Key::PbclLive));
        Status::Ok
    } else {
        log.warn(t(Key::PbclMismatch));
        pb_update_help(log);
        Status::Warn
    }
}

fn pb_update_help(log: &mut Log) {
    log.blank();
    log.header(t(Key::HdrHowToUpdate));
    log.plain(t(Key::UpdateStep1));
    log.plain(tf(Key::UpdateStep2Tmpl, &[BF3_APPID]));
    log.plain(t(Key::UpdateStep3));
}

/// The full interactive setup: OS spoof + PunkBuster check.
pub fn run_setup(log: &mut Log) -> (Status, Status) {
    let ua = do_ua(log);
    let pb = do_punkbuster(log);
    log.blank();
    log.ok(t(Key::DoneLaunch));
    (ua, pb)
}
