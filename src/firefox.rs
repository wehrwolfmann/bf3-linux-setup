//! Firefox profile detection and the `user.js` OS-spoof, ported from the
//! reference Python (`firefox_bases`, `default_profiles`, `set_ua`, `unset_ua`).

use crate::consts::{
    ua_prefs, CHROME_VERSION_FALLBACK, CHROME_VERSION_TIMEOUT_SECS, CHROME_VERSION_URL, MARKER,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// Candidate Firefox roots under a given home directory, in priority order:
/// standard, the nonstandard `~/.config/mozilla/firefox`, and the flatpak path.
/// Only roots that actually contain a `profiles.ini` are returned.
pub fn firefox_bases_in(home: &Path) -> Vec<PathBuf> {
    let candidates = [
        home.join(".mozilla/firefox"),
        home.join(".config/mozilla/firefox"),
        home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"),
    ];
    candidates
        .into_iter()
        .filter(|p| p.join("profiles.ini").is_file())
        .collect()
}

/// Same, rooted at the current user's home directory.
pub fn firefox_bases() -> Vec<PathBuf> {
    match home_dir() {
        Some(h) => firefox_bases_in(&h),
        None => Vec::new(),
    }
}

/// A parsed INI section: its name plus ordered key/value pairs.
struct Section {
    name: String,
    entries: Vec<(String, String)>,
}

impl Section {
    fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Minimal INI parser sufficient for `profiles.ini` (sections + `key=value`).
fn parse_ini(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            sections.push(Section {
                name: line[1..line.len() - 1].to_string(),
                entries: Vec::new(),
            });
        } else if let Some(eq) = line.find('=') {
            if let Some(sec) = sections.last_mut() {
                let key = line[..eq].trim().to_string();
                let val = line[eq + 1..].trim().to_string();
                sec.entries.push((key, val));
            }
        }
    }
    sections
}

/// The default profile directory for a Firefox base, following the same
/// precedence as the Python: Install-section `Default` → legacy `Default=1`
/// → a `*.default-release` profile → the first profile listed.
pub fn default_profiles(base: &Path) -> Vec<PathBuf> {
    let ini_path = base.join("profiles.ini");
    let text = match fs::read_to_string(&ini_path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    default_profiles_from_str(base, &text)
}

/// Testable core of [`default_profiles`] that works on raw INI text.
pub fn default_profiles_from_str(base: &Path, text: &str) -> Vec<PathBuf> {
    let sections = parse_ini(text);
    let mut install_default: Option<PathBuf> = None;
    let mut legacy_default: Option<PathBuf> = None;
    let mut release: Option<PathBuf> = None;
    let mut first: Option<PathBuf> = None;

    for sec in &sections {
        if sec.name.starts_with("Install") {
            if let Some(path) = sec.get("Default") {
                if !path.is_empty() {
                    install_default = Some(base.join(path));
                }
            }
        }
        if sec.name.starts_with("Profile") {
            let path = match sec.get("Path") {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };
            let prof = base.join(path);
            if first.is_none() {
                first = Some(prof.clone());
            }
            if sec.get("Default") == Some("1") {
                legacy_default = Some(prof.clone());
            }
            if path.contains("default-release") {
                release = Some(prof);
            }
        }
    }

    match install_default.or(legacy_default).or(release).or(first) {
        Some(p) => vec![p],
        None => Vec::new(),
    }
}

fn read_user_js(user_js: &Path) -> Vec<String> {
    match fs::read_to_string(user_js) {
        Ok(t) => t.lines().map(str::to_string).collect(),
        Err(_) => Vec::new(),
    }
}

fn strip_our_lines(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().filter(|l| !l.contains(MARKER)).collect()
}

fn write_user_js(user_js: &Path, lines: &[String]) {
    if user_js.is_file() {
        let _ = fs::copy(user_js, user_js.with_extension("js.bf3bak"));
    }
    let joined = lines.join("\n");
    let trimmed = joined.trim_end_matches('\n');
    let out = if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    };
    let _ = fs::write(user_js, out);
}

fn escape_pref(val: &str) -> String {
    val.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Where the Chrome version used to build the signature came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VersionSource {
    /// Read live from Google's public release catalogue.
    Online,
    /// Google's catalogue was unreachable or unusable; the built-in number was
    /// used instead.
    Fallback,
}

/// The Chrome version to put in the User-Agent: live from Google's catalogue
/// when reachable, otherwise the built-in fallback. Never fails, never blocks
/// longer than [`CHROME_VERSION_TIMEOUT_SECS`].
pub fn chrome_ua_version() -> (String, VersionSource) {
    match fetch_chrome_ua_version() {
        Some(v) => (v, VersionSource::Online),
        None => (CHROME_VERSION_FALLBACK.to_string(), VersionSource::Fallback),
    }
}

/// Ask Google's release catalogue for the newest stable Windows Chrome and
/// return it in User-Agent form. `None` on any network, TLS or parse failure —
/// callers must degrade to the fallback, never block.
pub fn fetch_chrome_ua_version() -> Option<String> {
    // ureq is built without a default TLS provider (rustls would drag in a
    // bundled MPL-2.0 root store); native-tls uses the system store and must be
    // wired up explicitly.
    let connector = ureq::native_tls::TlsConnector::new().ok()?;
    let timeout = Duration::from_secs(CHROME_VERSION_TIMEOUT_SECS);
    let body = ureq::builder()
        .timeout(timeout)
        // The connect phase is NOT covered by the overall timeout above: it has
        // its own limit, defaulting to 30 s, which wins. Without this line a
        // black-holed route (Wi-Fi associated but dead) stalls the setup for
        // half a minute.
        .timeout_connect(timeout)
        .tls_connector(Arc::new(connector))
        .build()
        .get(CHROME_VERSION_URL)
        .call()
        .ok()?
        .into_string()
        .ok()?;
    parse_chrome_ua_version(&body)
}

/// Pull the newest version out of the catalogue's JSON and normalise it to the
/// form Chrome actually reports.
///
/// The catalogue gives a full build number (`151.0.7922.72`); since Chrome 107
/// the browser itself freezes everything after the major in its User-Agent, so
/// a real Chrome sends `151.0.0.0`. We mirror that rather than leaking the
/// exact build. `None` if no plausible version is present.
pub fn parse_chrome_ua_version(json: &str) -> Option<String> {
    let raw = json_string_field(json, "version")?;
    let major: u32 = raw.split('.').next()?.parse().ok()?;
    // Sanity window: Chrome majors are three digits and rising. Anything else
    // means we misread the payload — better to fall back than to write junk.
    if !(100..=999).contains(&major) {
        return None;
    }
    Some(format!("{major}.0.0.0"))
}

/// Value of the first `"<key>": "<value>"` pair in a flat-enough JSON document.
/// Purpose-built for the one small, stable payload we read — no dependency and
/// no general-purpose parser needed.
fn json_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut rest = json;
    while let Some(pos) = rest.find(&needle) {
        let after = rest[pos + needle.len()..].trim_start();
        if let Some(value) = after.strip_prefix(':') {
            let value = value.trim_start();
            if let Some(open) = value.strip_prefix('"') {
                if let Some(end) = open.find('"') {
                    return Some(open[..end].to_string());
                }
            }
        }
        rest = &rest[pos + needle.len()..];
    }
    None
}

/// Write the four OS-spoof prefs into `<profile>/user.js` idempotently: any of
/// our previously written (marker-tagged) lines are stripped first, so a second
/// run never duplicates them. `windows_ua` is the signature built by
/// [`crate::consts::windows_ua`] around the current Chrome version.
pub fn set_ua(profile: &Path, windows_ua: &str) {
    let user_js = profile.join("user.js");
    let mut lines = strip_our_lines(read_user_js(&user_js));
    for (key, val) in ua_prefs(windows_ua) {
        let v = escape_pref(val);
        lines.push(format!("user_pref(\"{key}\", \"{v}\");  {MARKER}"));
    }
    write_user_js(&user_js, &lines);
}

/// Remove our marker-tagged lines from `user.js`. Returns true if anything was
/// removed.
pub fn unset_ua(profile: &Path) -> bool {
    let user_js = profile.join("user.js");
    let lines = read_user_js(&user_js);
    let stripped = strip_our_lines(lines.clone());
    if stripped.len() == lines.len() {
        return false;
    }
    write_user_js(&user_js, &stripped);
    true
}

/// Whether a Firefox process is currently running (a restart is needed for the
/// prefs to take effect).
pub fn firefox_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "firefox"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::windows_ua;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("bf3rs-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn profiles_install_default_wins() {
        let base = Path::new("/base");
        let ini = "\
[Install4F96D1932A9F858E]
Default=Profiles/abc.default-release
Locked=1

[Profile1]
Name=default
IsRelative=1
Path=Profiles/old.default
Default=1

[Profile0]
Name=default-release
IsRelative=1
Path=Profiles/abc.default-release
";
        let got = default_profiles_from_str(base, ini);
        assert_eq!(got, vec![base.join("Profiles/abc.default-release")]);
    }

    #[test]
    fn profiles_legacy_default_flag() {
        let base = Path::new("/base");
        let ini = "\
[Profile0]
Name=default
Path=xyz.default
Default=1

[Profile1]
Name=dev
Path=dev.profile
";
        let got = default_profiles_from_str(base, ini);
        assert_eq!(got, vec![base.join("xyz.default")]);
    }

    #[test]
    fn profiles_default_release_then_first() {
        let base = Path::new("/base");
        // No Install-default, no Default=1 → default-release wins.
        let ini_rel = "\
[Profile0]
Path=aaa.first

[Profile1]
Path=bbb.default-release
";
        assert_eq!(
            default_profiles_from_str(base, ini_rel),
            vec![base.join("bbb.default-release")]
        );

        // Nothing special → first profile.
        let ini_first = "\
[Profile0]
Path=only.profile
";
        assert_eq!(
            default_profiles_from_str(base, ini_first),
            vec![base.join("only.profile")]
        );
    }

    #[test]
    fn bases_detects_all_three_layouts() {
        for sub in [
            ".mozilla/firefox",
            ".config/mozilla/firefox",
            ".var/app/org.mozilla.firefox/.mozilla/firefox",
        ] {
            let home = temp_dir("home");
            let base = home.join(sub);
            fs::create_dir_all(&base).unwrap();
            fs::write(base.join("profiles.ini"), "[Profile0]\nPath=x\n").unwrap();
            let bases = firefox_bases_in(&home);
            assert_eq!(bases, vec![base], "layout {sub} not detected");
        }
    }

    #[test]
    fn set_ua_writes_exact_prefs_and_is_idempotent() {
        let prof = temp_dir("prof");
        let ua = windows_ua("151.0.0.0");
        set_ua(&prof, &ua);
        let user_js = prof.join("user.js");
        let text = fs::read_to_string(&user_js).unwrap();

        let expected = [
            format!("user_pref(\"general.useragent.override\", \"{ua}\");  {MARKER}"),
            format!("user_pref(\"general.platform.override\", \"Win32\");  {MARKER}"),
            format!("user_pref(\"general.oscpu.override\", \"Windows NT 10.0; Win64; x64\");  {MARKER}"),
            format!("user_pref(\"general.appversion.override\", \"5.0 (Windows)\");  {MARKER}"),
        ];
        for line in &expected {
            assert!(text.contains(line), "missing pref line: {line}\n--- got ---\n{text}");
        }
        assert_eq!(text.lines().count(), 4, "expected exactly 4 lines");

        // Second run must not duplicate.
        set_ua(&prof, &ua);
        let text2 = fs::read_to_string(&user_js).unwrap();
        assert_eq!(text2.lines().count(), 4, "second run duplicated lines");
        assert_eq!(text, text2, "second run changed content");

        // A backup was created on the second (existing-file) write.
        assert!(user_js.with_extension("js.bf3bak").is_file());

        // Undo removes exactly our lines.
        assert!(unset_ua(&prof));
        let after = fs::read_to_string(&user_js).unwrap();
        assert!(after.trim().is_empty(), "user.js not empty after undo: {after:?}");
        // Second undo is a no-op.
        assert!(!unset_ua(&prof));

        let _ = fs::remove_dir_all(&prof);
    }

    #[test]
    fn set_ua_preserves_foreign_lines() {
        let prof = temp_dir("prof2");
        let user_js = prof.join("user.js");
        fs::write(&user_js, "user_pref(\"browser.foo\", true);\n").unwrap();
        set_ua(&prof, &windows_ua("151.0.0.0"));
        let text = fs::read_to_string(&user_js).unwrap();
        assert!(text.contains("browser.foo"), "foreign pref lost");
        assert_eq!(text.lines().count(), 5); // 1 foreign + 4 ours
        assert!(unset_ua(&prof));
        let after = fs::read_to_string(&user_js).unwrap();
        assert_eq!(after.trim(), "user_pref(\"browser.foo\", true);");
        let _ = fs::remove_dir_all(&prof);
    }

    // Verbatim response of the public catalogue (captured 2026-08-04).
    const CATALOGUE_SAMPLE: &str = r#"{
  "versions": [
    {
      "name": "chrome/platforms/win/channels/stable/versions/151.0.7922.72",
      "version": "151.0.7922.72"
    }
  ],
  "nextPageToken": "1885305001"
}"#;

    #[test]
    fn parses_catalogue_and_reduces_to_the_reported_form() {
        // Chrome freezes everything after the major in its own User-Agent.
        assert_eq!(
            parse_chrome_ua_version(CATALOGUE_SAMPLE),
            Some("151.0.0.0".to_string())
        );
        assert_eq!(
            parse_chrome_ua_version(r#"{"versions":[{"version":"162.0.1.2"}]}"#),
            Some("162.0.0.0".to_string())
        );
    }

    #[test]
    fn rejects_missing_or_implausible_versions() {
        for junk in [
            "",
            "not json at all",
            r#"{"versions":[]}"#,
            r#"{"version":"abc"}"#,
            r#"{"version":""}"#,
            r#"{"version":"99.0.0.0"}"#, // impossibly old → misread payload
            r#"{"version":"1000.0.0.0"}"#, // out of the sane window
            r#"{"version":151}"#,        // not a string
        ] {
            assert_eq!(
                parse_chrome_ua_version(junk),
                None,
                "should reject: {junk:?}"
            );
        }
    }

    #[test]
    fn json_field_reader_ignores_lookalikes_and_earlier_keys() {
        // "versions/151…" inside the name must not be mistaken for the field.
        assert_eq!(
            json_string_field(CATALOGUE_SAMPLE, "version").as_deref(),
            Some("151.0.7922.72")
        );
        assert_eq!(json_string_field(r#"{"a":"b"}"#, "version"), None);
        assert_eq!(
            json_string_field(r#"{"version_note":"x","version":"7"}"#, "version").as_deref(),
            Some("7")
        );
    }

    #[test]
    #[ignore = "hits Google's live version catalogue; run with --ignored"]
    fn live_chrome_version_is_sane() {
        let (v, src) = chrome_ua_version();
        assert_eq!(src, VersionSource::Online, "catalogue fetch failed");
        let major: u32 = v.split('.').next().unwrap().parse().unwrap();
        assert!(major >= 151, "unexpectedly low online Chrome version: {v}");
        assert!(v.ends_with(".0.0.0"), "not in reported form: {v}");
    }
}
