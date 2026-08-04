# bf3-linux-setup

Get **Battlefield 3** (Steam/Proton) playable through Battlelog on Linux — one
small native binary, no scripting runtime, no browser extensions.

It fixes the two things that stop BF3 from launching on Linux:

1. **Battlelog "Windows Vista or later" block.** Battlelog decides whether you
   may launch the game from the OS reported by your browser's User-Agent. A
   native Linux Firefox reports "Linux" and Battlelog refuses. This tool writes
   four global preference overrides into Firefox's `user.js` so Battlelog sees
   Windows — no extension, no manual `about:config` editing.
2. **PunkBuster "outdated" kick.** PunkBuster kicks you if its client module is
   stale. The tool reads the current client version from the public Even
   Balance security file and, in launch-wrapper mode, refreshes that file inside
   the game folder so PunkBuster auto-updates itself from the PB servers on the
   next launch.

Everything it changes is reversible.

## The four Firefox overrides

A per-site User-Agent override was removed from Firefox in v71 and no longer
works, and the User-Agent alone is not enough — Battlelog also inspects
`navigator.platform`, `oscpu` and `appVersion`. So all four are set **globally**:

| Preference                      | Value                              |
| ------------------------------- | ---------------------------------- |
| `general.useragent.override`    | a current Windows Chrome UA        |
| `general.platform.override`     | `Win32`                            |
| `general.oscpu.override`        | `Windows NT 10.0; Win64; x64`      |
| `general.appversion.override`   | `5.0 (Windows)`                    |

Each line we add is tagged with a `# bf3-linux-setup` marker, so re-running is
idempotent (no duplicates) and removal only ever touches our own lines. The
existing `user.js` is backed up to `user.js.bf3bak` before the first write.

### The browser version is looked up, not frozen

Since these overrides are **global**, the signature you get is the one every
site sees — not just Battlelog. That matters, because a growing number of sites
answer an outdated browser signature with a bare `403` and no explanation, and
the cut-off moves up with every Chrome release. A User-Agent baked into the
binary would fix Battlelog today and silently break other sites a year later.

So the current stable Chrome version is read at run time from Google's public
release catalogue (`versionhistory.googleapis.com`, no key, no account) and
written into the signature — reduced to the `<major>.0.0.0` form Chrome itself
reports. The tool tells you which version it used and whether it came from the
catalogue or from the built-in fallback. If the lookup fails for any reason it
falls back to the version compiled in, after a short timeout; nothing hangs and
nothing aborts. The signature is plain Chrome with no `Edg/` suffix, so it never
advertises an Edge build that does not match the Chrome one.

## Usage

```
bf3-linux-setup                        open the setup window (default)
bf3-linux-setup --setup                run the setup in the terminal
bf3-linux-setup --ua-off               remove the Firefox overrides
bf3-linux-setup --steam-launch -- CMD  launch wrapper (see below)
bf3-linux-setup --help                 show this help
```

### Setup window

Run the binary with no arguments to open the window. Click **Fix BF3** to write
the Firefox overrides and check PunkBuster; the two status dots turn green when
each step is good. After it runs, fully restart Firefox, open Battlelog, and hit
**Join**. Start the game itself from Steam (Play). Use **Undo UA** to remove the
overrides again.

### Launch wrapper (keeps PunkBuster fresh automatically)

Click **Install Steam launch wrapper** in the window — it copies (and, with
Steam closed, tries to write) the following into BF3's Steam launch options:

```
"/path/to/bf3-linux-setup" --steam-launch -- %command%
```

From then on, every time you press Play, the wrapper refreshes the PunkBuster
security file (if the online version is newer than yours) and then hands off to
the real game, so PunkBuster self-updates each session. A network hiccup never
blocks the launch — the game always starts.

## Install

### Regular Linux

Download the prebuilt binary from the Releases page, make it executable, and run
it:

```
chmod +x bf3-linux-setup
./bf3-linux-setup
```

The binary is fully static against system GUI libraries (it uses its own
renderer), so there are no GTK/Qt dependencies to install.

### Steam Deck / Steam Machine

Switch to Desktop Mode. The system image is immutable, but this binary needs
nothing installed — just drop it in a normal folder (e.g. `~/bf3-linux-setup`),
mark it executable, and run it. If you use the Flatpak Firefox, its profile
(`~/.var/app/org.mozilla.firefox/.mozilla/firefox`) is detected automatically.
Then add the launch wrapper to BF3's launch options as shown above so
PunkBuster stays current after each Steam update.

Firefox profiles are discovered from `~/.mozilla/firefox`,
`~/.config/mozilla/firefox`, and the Flatpak path above, picking the
default-release profile from `profiles.ini`.

## Building from source

```
cargo build --release
# binary at target/release/bf3-linux-setup
```

Requires a stable Rust toolchain (edition 2021). Releases ship a prebuilt
binary alongside the source.

## Requirements

- Firefox (native or Flatpak).
- The Steam version of Battlefield 3 (AppID 1238820) with Proton.
- OpenSSL 3 (`libssl.so.3`), used for the HTTPS version lookup and linked from
  the system so it stays on your distribution's security updates. Present out of
  the box on current distributions and on SteamOS.
- For a full PunkBuster reinstall (rarely needed): `protontricks`. The tool
  prints the exact command if it detects a mismatch.

## License

MIT.
