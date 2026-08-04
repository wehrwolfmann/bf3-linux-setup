//! PunkBuster logic: locate the BF3 install, read the installed client version,
//! read the current version from Even Balance, and refresh the security file so
//! PunkBuster self-updates on the next game start. Ported from the reference
//! Python (`steam_libraries`, `find_bf3`, `punkbuster_latest_online`,
//! `punkbuster_status`) plus the new launch-wrapper refresh step.

use crate::consts::PB_SEC_URL;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Steam library roots, discovered from `libraryfolders.vdf` (both the flatpak
/// and classic locations). Falls back to the default Steam dir.
pub fn steam_libraries_in(home: &Path) -> Vec<PathBuf> {
    let mut libs: Vec<PathBuf> = Vec::new();
    let vdfs = [
        home.join(".local/share/Steam/steamapps/libraryfolders.vdf"),
        home.join(".steam/steam/steamapps/libraryfolders.vdf"),
    ];
    for vdf in vdfs {
        if let Ok(text) = fs::read_to_string(&vdf) {
            for p in extract_vdf_paths(&text) {
                libs.push(PathBuf::from(p));
            }
        }
    }
    if libs.is_empty() {
        libs.push(home.join(".local/share/Steam"));
    }
    // Dedup, preserving order.
    let mut seen: Vec<PathBuf> = Vec::new();
    for p in libs {
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    seen
}

/// Pull every `"path" "<value>"` entry out of a `libraryfolders.vdf`.
fn extract_vdf_paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        // A library entry looks like:  "path"  "/some/dir"
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("\"path\"") {
            if let Some(val) = first_quoted(rest) {
                out.push(unescape_vdf(&val));
            }
        }
    }
    out
}

/// Return the contents of the first double-quoted token in `s`.
fn first_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let rest = &s[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn unescape_vdf(s: &str) -> String {
    s.replace("\\\\", "\\")
}

/// Locate the BF3 install directory across all Steam libraries.
pub fn find_bf3_in(home: &Path) -> Option<PathBuf> {
    for lib in steam_libraries_in(home) {
        let game = lib.join("steamapps/common/Battlefield 3");
        if game.is_dir() {
            return Some(game);
        }
    }
    None
}

/// Locate BF3 for the current user.
pub fn find_bf3() -> Option<PathBuf> {
    home_dir().and_then(|h| find_bf3_in(&h))
}

/// Highest client (pbcl) version advertised by Even Balance, parsed from the
/// "F C <variant> <version>" lines of `pbsec.htm`. Returns `None` if no such
/// line is present.
pub fn parse_pbsec_client_version(text: &str) -> Option<u32> {
    let mut best: Option<u32> = None;
    for line in text.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        // Find an "F C <single-alnum> <number>" run anywhere in the line
        // (real lines are prefixed with an HTML "<p>" tag).
        for w in tokens.windows(4) {
            if w[0] == "F"
                && w[1] == "C"
                && w[2].len() == 1
                && w[2].chars().all(|c| c.is_ascii_alphanumeric())
            {
                if let Ok(v) = w[3].parse::<u32>() {
                    best = Some(best.map_or(v, |b| b.max(v)));
                }
            }
        }
    }
    best
}

/// Fetch and parse the current online client version. `None` on any network or
/// parse failure (callers must degrade gracefully, never block).
pub fn punkbuster_latest_online() -> Option<u32> {
    let body = fetch_pbsec()?;
    parse_pbsec_client_version(&body)
}

/// Download the raw `pbsec.htm` text (latin-1 tolerant). `None` on failure.
pub fn fetch_pbsec() -> Option<String> {
    let timeout = Duration::from_secs(15);
    let resp = ureq::builder()
        .timeout(timeout)
        // The connect phase has its own limit (default 30 s) that wins over the
        // overall timeout, so it has to be capped explicitly — the launch
        // wrapper must never stall the game behind a dead route.
        .timeout_connect(timeout)
        .build()
        .get(PB_SEC_URL)
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    use std::io::Read;
    resp.into_reader().read_to_end(&mut bytes).ok()?;
    // The file is ASCII/latin-1; decode losslessly as latin-1.
    Some(bytes.iter().map(|&b| b as char).collect())
}

/// The installed PunkBuster client version, read from `<bf3>/pb/dll/wc*.dll`
/// (the highest numbered client DLL). `None` if PunkBuster is absent.
pub fn installed_client_version(bf3: &Path) -> Option<(u32, PathBuf)> {
    let dir = bf3.join("pb/dll");
    let entries = fs::read_dir(&dir).ok()?;
    let mut best: Option<(u32, PathBuf)> = None;
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if let Some(v) = parse_wc_version(&name) {
            match &best {
                Some((bv, _)) if *bv >= v => {}
                _ => best = Some((v, e.path())),
            }
        }
    }
    best
}

/// Parse `wc0*<digits>.dll` (case-insensitive) into its numeric version.
fn parse_wc_version(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_prefix("wc")?.strip_suffix(".dll")?;
    if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    stem.parse::<u32>().ok()
}

/// Whether the active `pbcl.dll` matches the newest client DLL by size
/// (mirrors the Python's "applied" check).
pub fn pbcl_matches_newest(bf3: &Path, newest_file: &Path) -> bool {
    let pbcl = bf3.join("pb/pbcl.dll");
    match (fs::metadata(&pbcl), fs::metadata(newest_file)) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => false,
    }
}

/// Write fresh security data into `<bf3>/pb/pbsec.htm` so PunkBuster pulls the
/// latest auto-update info from the PB servers on the next launch. This is the
/// exact mechanism Even Balance documents on the page itself. Returns the path
/// written on success.
pub fn refresh_pbsec_file(bf3: &Path, body: &str) -> std::io::Result<PathBuf> {
    let pb = bf3.join("pb");
    fs::create_dir_all(&pb)?;
    let dest = pb.join("pbsec.htm");
    fs::write(&dest, body)?;
    Ok(dest)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("bf3rs-pb-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    // Real captured copy of the Even Balance page (read-only network snapshot).
    const PBSEC_SAMPLE: &str = include_str!("../tests/fixtures/pbsec_sample.htm");

    #[test]
    fn parses_client_version_from_real_sample() {
        // The live page's client (F C) lines are all v2352.
        assert_eq!(parse_pbsec_client_version(PBSEC_SAMPLE), Some(2352));
    }

    #[test]
    fn parses_client_version_picks_max() {
        let text = "\
<p> F A W 1386 E000 ...
<p> F C L 2352 11 ...
<p> F C W 2400 F8590 ...
<p> F S V 1903 AFE1C ...
";
        assert_eq!(parse_pbsec_client_version(text), Some(2400));
    }

    #[test]
    fn no_client_line_returns_none() {
        assert_eq!(parse_pbsec_client_version("<p> F A L 1386 11 ...\n"), None);
        assert_eq!(parse_pbsec_client_version("random text"), None);
    }

    #[test]
    fn wc_version_parsing() {
        assert_eq!(parse_wc_version("wc002352.dll"), Some(2352));
        assert_eq!(parse_wc_version("WC0042.DLL"), Some(42));
        assert_eq!(parse_wc_version("wc1234.dll"), Some(1234));
        assert_eq!(parse_wc_version("wc.dll"), None);
        assert_eq!(parse_wc_version("pbcl.dll"), None);
        assert_eq!(parse_wc_version("wcabc.dll"), None);
    }

    #[test]
    fn extracts_library_paths_from_vdf() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
	}
	"1"
	{
		"path"		"/mnt/games/SteamLibrary"
	}
}
"#;
        let paths = extract_vdf_paths(vdf);
        assert_eq!(
            paths,
            vec![
                "/home/u/.local/share/Steam".to_string(),
                "/mnt/games/SteamLibrary".to_string()
            ]
        );
    }

    #[test]
    fn installed_version_and_pbcl_match() {
        let bf3 = temp_dir("bf3");
        let dll = bf3.join("pb/dll");
        fs::create_dir_all(&dll).unwrap();
        fs::write(dll.join("wc001000.dll"), b"old-smaller").unwrap();
        fs::write(dll.join("wc002352.dll"), b"newest-client-bytes").unwrap();
        fs::write(dll.join("readme.txt"), b"ignore").unwrap();

        let (ver, newest) = installed_client_version(&bf3).unwrap();
        assert_eq!(ver, 2352);
        assert_eq!(newest.file_name().unwrap(), "wc002352.dll");

        // pbcl matches newest by size.
        fs::create_dir_all(bf3.join("pb")).unwrap();
        fs::write(bf3.join("pb/pbcl.dll"), b"newest-client-bytes").unwrap();
        assert!(pbcl_matches_newest(&bf3, &newest));
        fs::write(bf3.join("pb/pbcl.dll"), b"different").unwrap();
        assert!(!pbcl_matches_newest(&bf3, &newest));

        let _ = fs::remove_dir_all(&bf3);
    }

    #[test]
    fn refresh_writes_pbsec_file() {
        let bf3 = temp_dir("bf3refresh");
        let dest = refresh_pbsec_file(&bf3, "SECURITY-DATA").unwrap();
        assert_eq!(dest, bf3.join("pb/pbsec.htm"));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "SECURITY-DATA");
        let _ = fs::remove_dir_all(&bf3);
    }

    #[test]
    fn find_bf3_locates_install() {
        let home = temp_dir("home");
        let game = home
            .join(".local/share/Steam/steamapps/common/Battlefield 3");
        fs::create_dir_all(&game).unwrap();
        assert_eq!(find_bf3_in(&home), Some(game));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    #[ignore = "hits the live Even Balance server; run with --ignored"]
    fn live_online_version_is_sane() {
        let v = punkbuster_latest_online().expect("network fetch failed");
        assert!(v >= 2352, "unexpectedly low online version: {v}");
    }
}
