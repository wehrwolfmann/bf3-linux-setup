//! Technical values the tool writes or fetches: exact strings confirmed working
//! on a live machine — do not "improve" them. The one deliberate exception is
//! the browser version inside the User-Agent, which is looked up at run time
//! (see [`CHROME_VERSION_URL`]) because a pinned one goes stale and starts
//! getting the user 403'd on unrelated sites.

/// Marker appended to every line we add to `user.js`, so removal/idempotency
/// never touches lines the user wrote themselves.
pub const MARKER: &str = "# bf3-linux-setup";

/// Steam AppID of Battlefield 3.
pub const BF3_APPID: &str = "1238820";

/// Public Even Balance file listing the current PunkBuster versions for BF3.
/// Lines "F C <variant> <version>": type C = client (pbcl), the one whose
/// staleness triggers the "PunkBuster outdated" kick. Served over plain HTTP.
pub const PB_SEC_URL: &str = "http://www.evenbalance.com/downloads/bf3/pbsec.htm";

/// Google's public Chrome release catalogue: newest stable Windows build first.
/// No key, no quota, no authentication; HTTPS only.
pub const CHROME_VERSION_URL: &str =
    "https://versionhistory.googleapis.com/v1/chrome/platforms/win/channels/stable/versions?pageSize=1";

/// Chrome version baked into the binary, used only when the catalogue above is
/// unreachable. Kept reasonably current so an offline run still produces a
/// signature no worse than the day the binary was built.
///
/// Why this matters: sites increasingly reject stale browser signatures with a
/// bare 403 and no explanation, and the cut-off moves with every Chrome
/// release. A frozen User-Agent therefore fixes Battlelog today and silently
/// breaks other sites tomorrow — hence the online lookup, with this as a floor.
pub const CHROME_VERSION_FALLBACK: &str = "151.0.0.0";

/// Overall timeout for the Chrome-version lookup. Deliberately short: the
/// setup must never appear to hang because a network is missing.
pub const CHROME_VERSION_TIMEOUT_SECS: u64 = 8;

/// The Windows Chrome User-Agent reported to Battlelog, built around
/// `chrome_version` (a full four-part number such as `151.0.0.0`).
///
/// Deliberately a plain Chrome signature with no `Edg/` suffix: Edge ships on
/// its own schedule and has no comparable public version feed, so a pinned Edge
/// number next to a live Chrome number would be an obviously inconsistent pair
/// — exactly the shape bot filters look for. Battlelog only cares that the
/// reported OS is Windows.
pub fn windows_ua(chrome_version: &str) -> String {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/{chrome_version} Safari/537.36"
    )
}

/// The four GLOBAL Firefox preference overrides that make Battlelog see
/// Windows. A per-site UA override was removed from Firefox in v71 and no
/// longer works; and UA alone is not enough because Battlelog also reads
/// navigator.platform / oscpu / appVersion — hence all four, applied globally.
pub fn ua_prefs(windows_ua: &str) -> [(&'static str, &str); 4] {
    [
        ("general.useragent.override", windows_ua),
        ("general.platform.override", "Win32"),
        ("general.oscpu.override", "Windows NT 10.0; Win64; x64"),
        ("general.appversion.override", "5.0 (Windows)"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ua_carries_the_given_version_and_no_edge_suffix() {
        let ua = windows_ua("151.0.0.0");
        assert!(ua.contains("Chrome/151.0.0.0 Safari/537.36"), "{ua}");
        assert!(
            ua.starts_with("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
            "{ua}"
        );
        assert!(
            !ua.contains("Edg/"),
            "Edge suffix must not be present: {ua}"
        );
        assert!(!ua.contains("Linux") && !ua.contains("X11"), "{ua}");
    }

    #[test]
    fn prefs_expose_the_ua_and_the_three_fixed_overrides() {
        let ua = windows_ua(CHROME_VERSION_FALLBACK);
        let prefs = ua_prefs(&ua);
        assert_eq!(prefs[0], ("general.useragent.override", ua.as_str()));
        assert_eq!(prefs[1].1, "Win32");
        assert_eq!(prefs[2].1, "Windows NT 10.0; Win64; x64");
        assert_eq!(prefs[3].1, "5.0 (Windows)");
    }

    #[test]
    fn fallback_is_a_plausible_four_part_version() {
        let parts: Vec<&str> = CHROME_VERSION_FALLBACK.split('.').collect();
        assert_eq!(parts.len(), 4, "{CHROME_VERSION_FALLBACK}");
        let major: u32 = parts[0].parse().expect("major must be numeric");
        assert!(
            major >= 138,
            "fallback must not be older than the version it replaced"
        );
    }
}
