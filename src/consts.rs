//! Fixed values ported 1:1 from the reference Python tool. These are the exact
//! strings confirmed working on a live machine — do not "improve" them.

/// User-Agent reported to Battlelog so it treats the browser as Windows.
pub const WINDOWS_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 Edg/138.0.3351.55";

/// Marker appended to every line we add to `user.js`, so removal/idempotency
/// never touches lines the user wrote themselves.
pub const MARKER: &str = "# bf3-linux-setup";

/// Steam AppID of Battlefield 3.
pub const BF3_APPID: &str = "1238820";

/// Public Even Balance file listing the current PunkBuster versions for BF3.
/// Lines "F C <variant> <version>": type C = client (pbcl), the one whose
/// staleness triggers the "PunkBuster outdated" kick. Served over plain HTTP.
pub const PB_SEC_URL: &str = "http://www.evenbalance.com/downloads/bf3/pbsec.htm";

/// The four GLOBAL Firefox preference overrides that make Battlelog see
/// Windows. A per-site UA override was removed from Firefox in v71 and no
/// longer works; and UA alone is not enough because Battlelog also reads
/// navigator.platform / oscpu / appVersion — hence all four, applied globally.
pub const UA_PREFS: [(&str, &str); 4] = [
    ("general.useragent.override", WINDOWS_UA),
    ("general.platform.override", "Win32"),
    ("general.oscpu.override", "Windows NT 10.0; Win64; x64"),
    ("general.appversion.override", "5.0 (Windows)"),
];
