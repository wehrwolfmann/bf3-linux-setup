//! Building the Steam launch-options wrapper string and writing it into Steam's
//! per-user `localconfig.vdf` for the BF3 app. This is a NEW capability beyond
//! the Python tool: it makes PunkBuster refresh itself on every launch.

use crate::i18n::{tf, Key};
use std::fs;
use std::path::{Path, PathBuf};

/// The launch-options string the user (or we) put into BF3's Steam properties.
/// Steam substitutes `%command%` with the real game invocation, which our
/// wrapper then execs after refreshing PunkBuster.
pub fn launch_options_string(exe: &str) -> String {
    format!("\"{exe}\" --steam-launch -- %command%")
}

/// Absolute path to the currently running binary, for embedding in the launch
/// options.
pub fn current_exe_string() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("bf3-linux-setup"))
        .to_string_lossy()
        .into_owned()
}

/// Every per-user `localconfig.vdf` found under both the classic and flatpak
/// Steam roots.
pub fn localconfig_paths(home: &Path) -> Vec<PathBuf> {
    let roots = [
        home.join(".local/share/Steam/userdata"),
        home.join(".steam/steam/userdata"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/userdata"),
    ];
    let mut out = Vec::new();
    for root in roots {
        let entries = match fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for user in entries.flatten() {
            let cfg = user.path().join("config/localconfig.vdf");
            if cfg.is_file() && !out.contains(&cfg) {
                out.push(cfg);
            }
        }
    }
    out
}

/// Write the wrapper launch options into BF3's entry in every discovered
/// `localconfig.vdf`. Returns the list of files updated and a list of
/// human-readable skips/errors. Steam must be closed or it will overwrite the
/// file on exit — the caller is expected to warn about that.
pub fn install_launch_options(
    home: &Path,
    appid: &str,
    value: &str,
) -> (Vec<PathBuf>, Vec<String>) {
    let mut updated = Vec::new();
    let mut notes = Vec::new();
    for cfg in localconfig_paths(home) {
        let text = match fs::read_to_string(&cfg) {
            Ok(t) => t,
            Err(e) => {
                notes.push(tf(Key::ReadFailedTmpl, &[&cfg.display().to_string(), &e.to_string()]));
                continue;
            }
        };
        match set_launch_options(&text, appid, value) {
            Ok(new_text) => {
                if let Err(e) = fs::write(&cfg, new_text) {
                    notes.push(tf(Key::WriteFailedTmpl, &[&cfg.display().to_string(), &e.to_string()]));
                } else {
                    updated.push(cfg);
                }
            }
            Err(e) => notes.push(format!("{}: {e}", cfg.display())),
        }
    }
    (updated, notes)
}

fn escape_vdf(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Set `LaunchOptions` for `appid` inside a `localconfig.vdf` document, keeping
/// the rest of the file intact. Returns the rewritten document, or an error
/// describing why the app block could not be located.
///
/// Kept pure (operates on strings) so it can be unit-tested without touching a
/// real Steam install.
pub fn set_launch_options(vdf: &str, appid: &str, value: &str) -> Result<String, String> {
    let needle = format!("\"{appid}\"");
    // Find the app key line.
    let key_pos = vdf
        .find(&needle)
        .ok_or_else(|| format!("app block \"{appid}\" not found in localconfig.vdf"))?;
    // Opening brace of the app block.
    let brace_open = vdf[key_pos..]
        .find('{')
        .map(|i| key_pos + i)
        .ok_or_else(|| "malformed app block (no opening brace)".to_string())?;

    // Match braces to find the end of this app's block.
    let mut depth = 0usize;
    let mut block_end = None;
    for (i, ch) in vdf[brace_open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    block_end = Some(brace_open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let block_end = block_end.ok_or_else(|| "malformed app block (unbalanced braces)".to_string())?;

    let block = &vdf[brace_open..=block_end];
    let escaped = escape_vdf(value);
    let new_entry_body = format!("\"LaunchOptions\"\t\t\"{escaped}\"");

    // Indentation: reuse the app key's leading whitespace plus one tab level.
    let line_start = vdf[..key_pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let key_indent: String = vdf[line_start..key_pos]
        .chars()
        .take_while(|c| *c == '\t' || *c == ' ')
        .collect();
    let inner_indent = format!("{key_indent}\t");

    let new_block = if let Some(lo) = block.find("\"LaunchOptions\"") {
        // Replace the existing entry (key + its quoted value) in place.
        let after_key = lo + "\"LaunchOptions\"".len();
        let vstart_rel = block[after_key..]
            .find('"')
            .map(|i| after_key + i)
            .ok_or_else(|| "malformed LaunchOptions value".to_string())?;
        let vend_rel = value_end(&block[vstart_rel..])
            .map(|i| vstart_rel + i)
            .ok_or_else(|| "unterminated LaunchOptions value".to_string())?;
        let mut nb = String::new();
        nb.push_str(&block[..lo]);
        nb.push_str(&new_entry_body);
        nb.push_str(&block[vend_rel + 1..]);
        nb
    } else {
        // Insert a fresh entry right after the opening brace.
        let brace_rel = block.find('{').unwrap();
        let mut nb = String::new();
        nb.push_str(&block[..=brace_rel]);
        nb.push('\n');
        nb.push_str(&inner_indent);
        nb.push_str(&new_entry_body);
        nb.push_str(&block[brace_rel + 1..]);
        nb
    };

    let mut out = String::with_capacity(vdf.len() + new_block.len());
    out.push_str(&vdf[..brace_open]);
    out.push_str(&new_block);
    out.push_str(&vdf[block_end + 1..]);
    Ok(out)
}

/// Given a slice starting at a value's opening quote, return the index of its
/// closing quote (honouring `\"` escapes).
fn value_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    debug_assert_eq!(bytes[0], b'"');
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPID: &str = "1238820";

    fn sample(with_launch_options: bool) -> String {
        let lo = if with_launch_options {
            "\t\t\t\t\t\t\"LaunchOptions\"\t\t\"OLD -foo\"\n"
        } else {
            ""
        };
        format!(
            "\"UserLocalConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"440\"\n\t\t\t{{\n\t\t\t\t\"LastPlayed\"\t\t\"1\"\n\t\t\t}}\n\t\t\t\"{APPID}\"\n\t\t\t{{\n{lo}\t\t\t\t\"LastPlayed\"\t\t\"2\"\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
        )
    }

    #[test]
    fn inserts_launch_options_when_absent() {
        let vdf = sample(false);
        let value = "\"/home/u/bf3\" --steam-launch -- %command%";
        let out = set_launch_options(&vdf, APPID, value).unwrap();
        // Value is VDF-escaped.
        assert!(out.contains(r#""LaunchOptions"		"\"/home/u/bf3\" --steam-launch -- %command%""#));
        // Other app untouched, and the 440 block's LastPlayed still there.
        assert!(out.contains("\"440\""));
        assert!(out.contains("\"LastPlayed\"\t\t\"2\""));
        // Structure still balanced.
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    #[test]
    fn replaces_existing_launch_options() {
        let vdf = sample(true);
        assert!(vdf.contains("OLD -foo"));
        let value = "NEW %command%";
        let out = set_launch_options(&vdf, APPID, value).unwrap();
        assert!(!out.contains("OLD -foo"), "old value not replaced");
        assert!(out.contains(r#""LaunchOptions"		"NEW %command%""#));
        // Only one LaunchOptions key remains.
        assert_eq!(out.matches("\"LaunchOptions\"").count(), 1);
        assert!(out.contains("\"LastPlayed\"\t\t\"2\""));
    }

    #[test]
    fn missing_app_block_errors() {
        let vdf = sample(false);
        assert!(set_launch_options(&vdf, "999999", "x").is_err());
    }

    #[test]
    fn launch_options_string_format() {
        assert_eq!(
            launch_options_string("/opt/bf3-linux-setup"),
            "\"/opt/bf3-linux-setup\" --steam-launch -- %command%"
        );
    }
}
