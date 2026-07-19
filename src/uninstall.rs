//! Uninstall — removing this tool's own installed footprint, behind a trait so
//! the destructive step is testable without ever touching a real install.
//!
//! What is removed (mirrors what [`crate::launcher`] installs, plus the binary
//! when it lives in the standard `~/.local/bin` location):
//!   1. the app-menu launcher  `~/.local/share/applications/bf3-linux-setup.desktop`
//!   2. the icon               `~/.local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg`
//!   3. the binary             `~/.local/bin/bf3-linux-setup` (only that path)
//!
//! The BF3 fixes themselves (Firefox spoof, Steam launch options, PunkBuster)
//! are NOT touched here — undoing the Firefox spoof is a separate, opt-in step
//! the GUI offers via `actions::do_ua_off`.
//!
//! Layer split (same shape as the dictation panel, so it is safe to test):
//!   - [`Layout`]   — where things live, built from `$HOME`;
//!   - [`plan`]     — PURE: computes the target list, no filesystem;
//!   - [`Executor`] — the "do it" trait: [`RealExecutor`] deletes for real,
//!     the test mock records calls and deletes nothing;
//!   - [`execute`]  — runs a plan through an executor.
//!
//! ⚠️ The user must confirm in the UI before [`execute`] runs with the real
//! executor. Tests use only the mock (or a temp `$HOME`); a live uninstall is
//! never run during verification.

use crate::launcher::{DESKTOP_BASENAME, ICON_BASENAME};
use std::path::{Path, PathBuf};

/// Binary basename, as installed to `~/.local/bin`.
pub const BIN_BASENAME: &str = "bf3-linux-setup";

/// Where this tool's own files live. Built from `$HOME` (XDG user dirs).
#[derive(Debug, Clone)]
pub struct Layout {
    /// `~/.local/bin/bf3-linux-setup`
    pub bin: PathBuf,
    /// `~/.local/share/applications/bf3-linux-setup.desktop`
    pub desktop: PathBuf,
    /// `~/.local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg`
    pub icon: PathBuf,
}

impl Layout {
    /// Standard layout for a home directory.
    pub fn from_home(home: &Path) -> Self {
        Layout {
            bin: home.join(".local/bin").join(BIN_BASENAME),
            desktop: home.join(".local/share/applications").join(DESKTOP_BASENAME),
            icon: home
                .join(".local/share/icons/hicolor/scalable/apps")
                .join(ICON_BASENAME),
        }
    }
}

/// The list of files an uninstall will remove.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallPlan {
    pub files: Vec<PathBuf>,
}

/// Compute the uninstall plan. Pure — removes nothing.
///
/// The binary is only ever targeted at the standard `~/.local/bin` path (never
/// wherever the process happens to be running from), so uninstalling a copy run
/// out of a build/download dir can't delete that arbitrary file.
pub fn plan(layout: &Layout) -> UninstallPlan {
    UninstallPlan {
        files: vec![
            layout.desktop.clone(),
            layout.icon.clone(),
            layout.bin.clone(),
        ],
    }
}

/// The destructive step. The real one deletes files; the test mock does not.
pub trait Executor {
    /// Remove a file. A missing file is not an error (partial install).
    fn remove_file(&self, path: &Path) -> Result<(), String>;
}

/// Run a plan through an executor.
pub fn execute(plan: &UninstallPlan, exec: &dyn Executor) -> Result<(), String> {
    for f in &plan.files {
        exec.remove_file(f)?;
    }
    Ok(())
}

/// Real executor: actually deletes files.
///
/// ⚠️ Only invoked after the user confirms in the UI.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealExecutor;

impl Executor for RealExecutor {
    fn remove_file(&self, path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

/// User home directory (shared with launcher).
pub fn home_dir() -> PathBuf {
    crate::launcher::home_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn plan_targets_launcher_icon_and_bin() {
        let home = Path::new("/home/u");
        let layout = Layout::from_home(home);
        let p = plan(&layout);
        assert!(p.files.contains(&home.join(".local/share/applications/bf3-linux-setup.desktop")));
        assert!(p.files.contains(
            &home.join(".local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg")
        ));
        assert!(p.files.contains(&home.join(".local/bin/bf3-linux-setup")));
        assert_eq!(p.files.len(), 3);
    }

    #[test]
    fn all_targets_stay_under_home() {
        // Guard: an uninstall must never target anything outside $HOME.
        let home = Path::new("/home/u");
        let layout = Layout::from_home(home);
        let p = plan(&layout);
        for f in &p.files {
            assert!(f.starts_with(home), "target {} escapes HOME", f.display());
        }
    }

    /// Mock executor: records calls, deletes NOTHING. Lets the destructive path
    /// be verified without risking the real system.
    #[derive(Default)]
    struct MockExec {
        calls: RefCell<Vec<String>>,
    }
    impl Executor for MockExec {
        fn remove_file(&self, path: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push(format!("rm {}", path.display()));
            Ok(())
        }
    }

    #[test]
    fn execute_removes_every_target_via_mock() {
        let layout = Layout::from_home(Path::new("/home/u"));
        let p = plan(&layout);
        let m = MockExec::default();
        execute(&p, &m).unwrap();

        let calls = m.calls.borrow();
        assert!(calls.contains(&"rm /home/u/.local/share/applications/bf3-linux-setup.desktop".to_string()));
        assert!(calls.contains(&"rm /home/u/.local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg".to_string()));
        assert!(calls.contains(&"rm /home/u/.local/bin/bf3-linux-setup".to_string()));
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn execute_against_temp_home_removes_only_temp_files() {
        // Real file removal, but on a throwaway $HOME — nothing real is touched.
        let tmp = std::env::temp_dir().join(format!("bf3-uninstall-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let layout = Layout::from_home(&tmp);

        for f in [&layout.bin, &layout.desktop, &layout.icon] {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, b"x").unwrap();
        }

        let p = plan(&layout);
        execute(&p, &RealExecutor).unwrap();

        assert!(!layout.bin.exists());
        assert!(!layout.desktop.exists());
        assert!(!layout.icon.exists());

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn missing_files_are_not_an_error() {
        // Partial install: nothing on disk, uninstall still succeeds.
        let tmp = std::env::temp_dir().join(format!("bf3-uninstall-empty-{}", std::process::id()));
        let layout = Layout::from_home(&tmp);
        let p = plan(&layout);
        assert!(execute(&p, &RealExecutor).is_ok());
    }
}
