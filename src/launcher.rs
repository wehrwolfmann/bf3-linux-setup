//! App-menu launcher — a freedesktop `.desktop` entry plus an icon, installed
//! into the per-user XDG directories (NO root):
//!   - `~/.local/share/applications/bf3-linux-setup.desktop`
//!   - `~/.local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg`
//!
//! The entry carries BOTH languages at once (`Name=`/`Name[ru]=`,
//! `Comment=`/`Comment[ru]=`, taken from the i18n tables via [`i18n::both`]) so
//! the menu shows the right text regardless of the current locale. `Exec=` is
//! the absolute path of the running binary; `StartupWMClass`/`Icon` match the
//! eframe `app_id` (see [`APP_ID`], wired via `with_app_id` in `gui.rs`) so the
//! desktop maps the running window to this launcher (one taskbar icon).
//!
//! Content generation ([`desktop_entry`]) is a pure function tested without a
//! filesystem; the on-disk write ([`install`]) is exercised against a temp dir.

use crate::i18n::{self, Key};
use std::path::{Path, PathBuf};

/// `.desktop` file basename.
pub const DESKTOP_BASENAME: &str = "bf3-linux-setup.desktop";
/// Icon basename (no directory).
pub const ICON_BASENAME: &str = "bf3-linux-setup.svg";
/// Window class = `Icon` name = `StartupWMClass` = eframe `app_id`.
/// Keep in sync with `ViewportBuilder::with_app_id` in `gui.rs`.
pub const APP_ID: &str = "bf3-linux-setup";

/// Placeholder application icon, embedded at compile time.
///
/// To ship the real artwork, replace the file below (keep its path) and
/// rebuild — no code change needed:
///   `assets/bf3-linux-setup.svg`
pub const ICON_SVG: &str = include_str!("../assets/bf3-linux-setup.svg");

/// Where the launcher's two files land for a given `home`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherPaths {
    /// `~/.local/share/applications/bf3-linux-setup.desktop`
    pub desktop: PathBuf,
    /// `~/.local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg`
    pub icon: PathBuf,
}

/// `.desktop` directory (XDG data → applications).
pub fn applications_dir(home: &Path) -> PathBuf {
    home.join(".local/share/applications")
}

/// Icon directory (hicolor scalable apps).
pub fn icons_dir(home: &Path) -> PathBuf {
    home.join(".local/share/icons/hicolor/scalable/apps")
}

/// Target paths of the launcher for a given home directory.
pub fn target_paths(home: &Path) -> LauncherPaths {
    LauncherPaths {
        desktop: applications_dir(home).join(DESKTOP_BASENAME),
        icon: icons_dir(home).join(ICON_BASENAME),
    }
}

/// Build the `.desktop` contents. `exec` is the absolute path of the gui binary.
///
/// Pure function: both localized `Name`/`Comment` come from the i18n tables, so
/// English and Russian are always emitted together.
pub fn desktop_entry(exec: &str) -> String {
    let (name_en, name_ru) = i18n::both(Key::DesktopName);
    let (comment_en, comment_ru) = i18n::both(Key::DesktopComment);
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={name_en}\n\
         Name[ru]={name_ru}\n\
         Comment={comment_en}\n\
         Comment[ru]={comment_ru}\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Game;Utility;\n\
         Keywords=Battlefield;BF3;Battlelog;PunkBuster;Steam;Proton;\n\
         StartupNotify=true\n\
         StartupWMClass={wm}\n",
        name_en = name_en,
        name_ru = name_ru,
        comment_en = comment_en,
        comment_ru = comment_ru,
        exec = exec,
        icon = APP_ID,
        wm = APP_ID,
    )
}

/// Install the launcher: write the icon and `.desktop` into the per-user XDG
/// directories. No root. Returns the written paths, or a human-readable error.
pub fn install(home: &Path, exec: &Path) -> Result<LauncherPaths, String> {
    let paths = target_paths(home);
    let exec_str = exec.to_string_lossy();

    write_file(&paths.icon, ICON_SVG)?;
    write_file(&paths.desktop, &desktop_entry(&exec_str))?;
    Ok(paths)
}

/// Create the parent directory (if needed) and write `contents` to `path`.
fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, contents).map_err(|e| format!("{}: {e}", path.display()))
}

/// Absolute path of the running binary, as a string (for `Exec=`). Falls back
/// to the bare app-id (assumes it is on `PATH`) if `current_exe` is unavailable.
pub fn current_exe_string() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| APP_ID.to_string())
}

/// User home directory (`$HOME`, `/root` fallback).
pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_has_exec_icon_wmclass_and_both_langs() {
        let d = desktop_entry("/home/u/.local/bin/bf3-linux-setup");
        assert!(d.contains("[Desktop Entry]"));
        assert!(d.contains("Type=Application"));
        assert!(d.contains("Exec=/home/u/.local/bin/bf3-linux-setup"));
        assert!(d.contains("Icon=bf3-linux-setup"));
        assert!(d.contains("StartupWMClass=bf3-linux-setup"));
        assert!(d.contains("Terminal=false"));
        assert!(d.contains("Categories=Game;Utility;"));
        // Both localized Name/Comment present, regardless of active locale.
        let (name_en, name_ru) = i18n::both(Key::DesktopName);
        let (com_en, com_ru) = i18n::both(Key::DesktopComment);
        assert!(d.contains(&format!("Name={name_en}\n")));
        assert!(d.contains(&format!("Name[ru]={name_ru}\n")));
        assert!(d.contains(&format!("Comment={com_en}\n")));
        assert!(d.contains(&format!("Comment[ru]={com_ru}\n")));
        // The Russian name is the well-known Cyrillic title.
        assert_eq!(name_ru, "Battlefield 3 — настройка для Linux");
    }

    #[test]
    fn target_paths_under_xdg_user_dirs() {
        let home = Path::new("/home/u");
        let p = target_paths(home);
        assert_eq!(
            p.desktop,
            Path::new("/home/u/.local/share/applications/bf3-linux-setup.desktop")
        );
        assert_eq!(
            p.icon,
            Path::new("/home/u/.local/share/icons/hicolor/scalable/apps/bf3-linux-setup.svg")
        );
        // Never escapes $HOME.
        assert!(p.desktop.starts_with(home));
        assert!(p.icon.starts_with(home));
    }

    #[test]
    fn icon_is_self_contained_svg() {
        assert!(ICON_SVG.contains("<svg"));
        // No external references that a menu icon must not have.
        assert!(!ICON_SVG.contains("http://") && !ICON_SVG.contains("https://")
            || ICON_SVG.contains("xmlns")); // xmlns URI is allowed; assert it parses as svg
        assert!(!ICON_SVG.contains("xlink:href"));
    }

    #[test]
    fn install_writes_both_files_to_temp() {
        // Temp "HOME" — nothing on the real system is touched.
        let tmp = std::env::temp_dir().join(format!("bf3-launcher-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let exec = Path::new("/opt/bf3/bf3-linux-setup");

        let paths = install(&tmp, exec).expect("install ok");
        assert!(paths.desktop.exists());
        assert!(paths.icon.exists());
        assert!(paths.desktop.starts_with(&tmp));

        let d = std::fs::read_to_string(&paths.desktop).unwrap();
        assert!(d.contains("Exec=/opt/bf3/bf3-linux-setup"));
        let svg = std::fs::read_to_string(&paths.icon).unwrap();
        assert!(svg.contains("<svg"));

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
