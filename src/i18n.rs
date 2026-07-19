//! Tiny two-language (English / Russian) UI layer.
//!
//! The language is chosen once, at first use, from the environment:
//!   `BF3_LANG` override → `LC_ALL` → `LC_MESSAGES` → `LANG`.
//! A value whose language part starts with `ru` (case-insensitive, e.g.
//! `ru_RU.UTF-8`) selects Russian; everything else — including an empty or
//! unset environment — falls back to English.
//!
//! Only human-facing display text lives here. Fixed technical values (the
//! Firefox UA/prefs, package/pref values, the Even Balance URL, file paths and
//! CLI flag names) are never routed through this module.

use std::sync::OnceLock;

/// The two supported UI languages. English is the default/fallback.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Ru,
}

/// Map a raw locale string (the value of `LANG`/`LC_*`) to a [`Lang`].
///
/// Russian iff the string starts with `ru` (case-insensitive), matching the
/// language part of values like `ru`, `ru_RU`, `ru_RU.UTF-8`, `RU.utf8`.
/// Anything else — other languages, empty, or garbage — is English.
pub fn lang_from_locale(raw: &str) -> Lang {
    let s = raw.trim();
    if s.len() >= 2 && s[..2].eq_ignore_ascii_case("ru") {
        // Guard against a hypothetical "rue"/"run…" false positive: require the
        // 2-letter code to be followed by a locale separator or end of string.
        match s[2..].chars().next() {
            None | Some('_') | Some('-') | Some('.') | Some('@') => Lang::Ru,
            _ => Lang::En,
        }
    } else {
        Lang::En
    }
}

/// Detect the UI language from the process environment. Order:
/// `BF3_LANG` (explicit override) → `LC_ALL` → `LC_MESSAGES` → `LANG`.
fn detect_lang() -> Lang {
    if let Ok(v) = std::env::var("BF3_LANG") {
        let v = v.trim();
        if !v.is_empty() {
            // Override accepts an explicit "ru"/"en" as well as a full locale.
            return lang_from_locale(v);
        }
    }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return lang_from_locale(&v);
            }
        }
    }
    Lang::En
}

/// The active UI language, detected once and cached for the process lifetime.
pub fn current() -> Lang {
    static CURRENT: OnceLock<Lang> = OnceLock::new();
    *CURRENT.get_or_init(detect_lang)
}

/// Pick the string for the active language.
fn pick(en: &'static str, ru: &'static str) -> &'static str {
    match current() {
        Lang::En => en,
        Lang::Ru => ru,
    }
}

/// Every user-facing message. English and Russian are declared side by side so
/// the exhaustive match guarantees both tables always carry the same keys.
#[derive(Clone, Copy)]
pub enum Key {
    // ── App / window ──────────────────────────────────────────────
    AppTitle,
    GuiSubtitle,
    GuiHint,
    // ── Status dots / buttons / hovers ────────────────────────────
    DotUa,
    DotPunkbuster,
    BtnFix,
    BtnUndo,
    BtnInstallWrapper,
    HoverInstallWrapper,
    // ── Launch-wrapper note (GUI) ─────────────────────────────────
    NoteCopiedTmpl,      // {0} = options
    NoteWrittenAuto,
    NoteCouldNotWrite,
    NoteReasonTmpl,      // {0} = reason
    NotePasteManually,
    // ── CLI: usage / errors ───────────────────────────────────────
    UsageBlock,
    GuiOpenFailedTmpl,   // {0} = error
    UnknownOptionTmpl,   // {0} = option
    // ── actions: User-Agent step ──────────────────────────────────
    HdrUa,
    NoFirefoxProfile,
    OsSpoofWrittenTmpl,  // {0} = profile name
    NoDefaultProfile,
    FirefoxRunningRestart,
    LaunchFirefoxHint,
    // ── actions: undo ─────────────────────────────────────────────
    HdrRemovingUa,
    UaRemovedTmpl,       // {0} = profile name
    NoChangesToRemove,
    RestartFirefoxNormalUa,
    // ── actions: PunkBuster ───────────────────────────────────────
    HdrPunkbuster,
    Bf3NotFound,
    GameTmpl,            // {0} = path
    PbFolderMissing,
    NoPbClientDlls,
    InstalledVersionTmpl,  // {0} = version
    CheckingOnline,
    OnlineUnreadable,
    NewerAvailableTmpl,    // {0} = online, {1} = installed
    UpToDateTmpl,          // {0} = online
    PbclLive,
    PbclMismatch,
    // ── actions: how-to-update block ──────────────────────────────
    HdrHowToUpdate,
    UpdateStep1,
    UpdateStep2Tmpl,       // {0} = appid
    UpdateStep3,
    // ── actions: final line ───────────────────────────────────────
    DoneLaunch,
    CliSetupHeader,
    // ── wrapper (stderr) ──────────────────────────────────────────
    WrapNoGameCmd,
    WrapLaunchFailedTmpl,  // {0} = program, {1} = error
    WrapBf3NotFound,
    WrapNoEvenBalance,
    WrapRefreshedTmpl,     // {0} = path, {1} = online version
    WrapWriteFailedTmpl,   // {0} = error
    WrapAlreadyCurrent,
    // ── steam_launch (note reasons) ───────────────────────────────
    ReadFailedTmpl,        // {0} = path, {1} = error
    WriteFailedTmpl,       // {0} = path, {1} = error
    // ── app launcher (.desktop) ───────────────────────────────────
    DesktopName,           // freedesktop Name= (also emitted as Name[ru])
    DesktopComment,        // freedesktop Comment= (also emitted as Comment[ru])
    BtnInstallLauncher,
    HoverInstallLauncher,
    LauncherInstalledTmpl, // {0} = .desktop path
    LauncherFailedTmpl,    // {0} = error
    // ── uninstall ─────────────────────────────────────────────────
    BtnUninstall,
    HoverUninstall,
    ConfirmUninstallTitle,
    ConfirmUninstallBody,
    ChkUndoUaSpoof,
    ConfirmDelete,
    ConfirmCancel,
    UninstallDone,
    UninstallFailedTmpl,   // {0} = error
}

/// The English string for a key. Templates use `{}` placeholders filled by
/// [`tf`]; positional notes above document their order.
fn en(k: Key) -> &'static str {
    use Key::*;
    match k {
        AppTitle => "Battlefield 3 — Linux setup",
        GuiSubtitle => "Battlelog via Firefox OS-spoof + PunkBuster refresh. Everything is reversible.",
        GuiHint => "Click \"Fix BF3\" to write the OS spoof and check PunkBuster.",
        DotUa => "User-Agent / OS spoof",
        DotPunkbuster => "PunkBuster",
        BtnFix => "▶  Fix BF3  (User-Agent + PunkBuster)",
        BtnUndo => "Undo UA",
        BtnInstallWrapper => "Install Steam launch wrapper",
        HoverInstallWrapper => "Copies (and tries to set) BF3's Steam launch options so PunkBuster self-updates each session",
        NoteCopiedTmpl => "Launch options copied to clipboard:\n    {}\n\nPaste them into Steam → Battlefield 3 → Properties → Launch Options.",
        NoteWrittenAuto => "\n\nWritten automatically into (close Steam first, or it will overwrite):",
        NoteCouldNotWrite => "\n\nCould not write it automatically",
        NoteReasonTmpl => " ({})",
        NotePasteManually => " — paste it manually.",
        UsageBlock => "Battlefield 3 — Linux setup\n\n\
             Usage:\n  \
             bf3-linux-setup                       open the setup window (default)\n  \
             bf3-linux-setup --setup               run the setup in the terminal\n  \
             bf3-linux-setup --ua-off              remove the Firefox OS spoof\n  \
             bf3-linux-setup --steam-launch -- CMD launch wrapper (put in Steam launch options)\n  \
             bf3-linux-setup --help                this help",
        GuiOpenFailedTmpl => "Could not open the GUI ({}). Try --setup for the terminal version.",
        UnknownOptionTmpl => "Unknown option: {}",
        HdrUa => "[1] User-Agent for Battlelog",
        NoFirefoxProfile => "No Firefox profile found. Is Firefox installed?",
        OsSpoofWrittenTmpl => "OS spoof (User-Agent/platform/oscpu) written: {}",
        NoDefaultProfile => "No default profile detected in profiles.ini.",
        FirefoxRunningRestart => "Firefox is running — RESTART it so the settings take effect.",
        LaunchFirefoxHint => "Launch Firefox, open Battlelog — the OS check should go green.",
        HdrRemovingUa => "Removing the User-Agent spoof",
        UaRemovedTmpl => "UA spoof removed: {}",
        NoChangesToRemove => "No changes of ours found in user.js — nothing to remove.",
        RestartFirefoxNormalUa => "Restart Firefox to restore the normal User-Agent.",
        HdrPunkbuster => "[2] PunkBuster",
        Bf3NotFound => "Battlefield 3 not found in your Steam libraries.",
        GameTmpl => "Game: {}",
        PbFolderMissing => "PunkBuster folder not found — anticheat not installed?",
        NoPbClientDlls => "No PunkBuster client DLLs found.",
        InstalledVersionTmpl => "Installed client version: v{}",
        CheckingOnline => "Checking the current version on the Even Balance server…",
        OnlineUnreadable => "Could not read the online version (no network / server down) — comparing locally only.",
        NewerAvailableTmpl => "A NEWER version is available online: v{} (you have v{}). Update PunkBuster.",
        UpToDateTmpl => "Your PunkBuster is UP TO DATE (latest online is v{}).",
        PbclLive => "Active pbcl.dll = newest version, anticheat is live.",
        PbclMismatch => "Active pbcl.dll does not match the newest local DLL — reinstall via pbsetup.",
        HdrHowToUpdate => "How to update PunkBuster:",
        UpdateStep1 => "  1. Download pbsetup.exe from evenbalance.com/downloads/pbsetup/pbsetup.zip → unzip into ~/Downloads/",
        UpdateStep2Tmpl => "  2. protontricks-launch --appid {} ~/Downloads/pbsetup.exe",
        UpdateStep3 => "  3. In pbsetup: \"Add a game\" → Battlefield 3 → \"Add Game\".",
        DoneLaunch => "Done. Launch BF3 from Steam (Play), open Battlelog, hit \"Join\".",
        CliSetupHeader => "Battlefield 3 — Linux setup",
        WrapNoGameCmd => "bf3-linux-setup: no game command given after `--`",
        WrapLaunchFailedTmpl => "bf3-linux-setup: failed to launch game ({}): {}",
        WrapBf3NotFound => "bf3-linux-setup: BF3 install not found; skipping PunkBuster refresh.",
        WrapNoEvenBalance => "bf3-linux-setup: could not reach Even Balance; skipping PunkBuster refresh.",
        WrapRefreshedTmpl => "bf3-linux-setup: refreshed PunkBuster security file → {} (online v{}); PunkBuster will auto-update on launch.",
        WrapWriteFailedTmpl => "bf3-linux-setup: could not write pbsec.htm: {}",
        WrapAlreadyCurrent => "bf3-linux-setup: PunkBuster already current; no refresh needed.",
        ReadFailedTmpl => "{}: read failed: {}",
        WriteFailedTmpl => "{}: write failed: {}",
        DesktopName => "Battlefield 3 Linux Setup",
        DesktopComment => "Set up Battlefield 3 for Battlelog on Linux: Firefox OS spoof + PunkBuster refresh",
        BtnInstallLauncher => "Add to app menu",
        HoverInstallLauncher => "Installs a desktop launcher and icon under ~/.local (no root) so this tool shows up in your application menu",
        LauncherInstalledTmpl => "Added to the application menu: {}",
        LauncherFailedTmpl => "Could not add the launcher: {}",
        BtnUninstall => "Uninstall…",
        HoverUninstall => "Removes this tool's launcher, icon and (if installed there) its binary from ~/.local",
        ConfirmUninstallTitle => "Uninstall",
        ConfirmUninstallBody => "Really remove this tool's app-menu launcher, icon and installed binary? Your BF3 fixes stay in place.",
        ChkUndoUaSpoof => "Also undo the Firefox OS spoof (User-Agent)",
        ConfirmDelete => "Delete",
        ConfirmCancel => "Cancel",
        UninstallDone => "Removed. The launcher, icon and installed binary are gone.",
        UninstallFailedTmpl => "Uninstall failed: {}",
    }
}

/// The Russian string for a key. Same set of variants as [`en`] (the exhaustive
/// match is what enforces one-to-one key parity between the two tables).
fn ru(k: Key) -> &'static str {
    use Key::*;
    match k {
        AppTitle => "Battlefield 3 — настройка для Linux",
        GuiSubtitle => "Battlelog через подмену ОС в Firefox + обновление PunkBuster. Все изменения обратимы.",
        GuiHint => "Нажмите «Исправить BF3», чтобы записать подмену ОС и проверить PunkBuster.",
        DotUa => "User-Agent / подмена ОС",
        DotPunkbuster => "PunkBuster",
        BtnFix => "▶  Исправить BF3  (User-Agent + PunkBuster)",
        BtnUndo => "Отменить UA",
        BtnInstallWrapper => "Установить обёртку запуска Steam",
        HoverInstallWrapper => "Копирует (и пытается прописать) параметры запуска BF3 в Steam, чтобы PunkBuster сам обновлялся при каждом запуске",
        NoteCopiedTmpl => "Параметры запуска скопированы в буфер обмена:\n    {}\n\nВставьте их в Steam → Battlefield 3 → Свойства → Параметры запуска.",
        NoteWrittenAuto => "\n\nЗаписано автоматически в (сначала закройте Steam, иначе он перезапишет файл):",
        NoteCouldNotWrite => "\n\nНе удалось записать автоматически",
        NoteReasonTmpl => " ({})",
        NotePasteManually => " — вставьте вручную.",
        UsageBlock => "Battlefield 3 — настройка для Linux\n\n\
             Использование:\n  \
             bf3-linux-setup                       открыть окно настройки (по умолчанию)\n  \
             bf3-linux-setup --setup               выполнить настройку в терминале\n  \
             bf3-linux-setup --ua-off              убрать подмену ОС в Firefox\n  \
             bf3-linux-setup --steam-launch -- CMD обёртка запуска (для параметров запуска Steam)\n  \
             bf3-linux-setup --help                эта справка",
        GuiOpenFailedTmpl => "Не удалось открыть графический интерфейс ({}). Попробуйте --setup для терминальной версии.",
        UnknownOptionTmpl => "Неизвестный параметр: {}",
        HdrUa => "[1] User-Agent для Battlelog",
        NoFirefoxProfile => "Профиль Firefox не найден. Firefox установлен?",
        OsSpoofWrittenTmpl => "Подмена ОС (User-Agent/platform/oscpu) записана: {}",
        NoDefaultProfile => "В profiles.ini не найден профиль по умолчанию.",
        FirefoxRunningRestart => "Firefox запущен — ПЕРЕЗАПУСТИТЕ его, чтобы настройки вступили в силу.",
        LaunchFirefoxHint => "Запустите Firefox, откройте Battlelog — проверка ОС должна стать зелёной.",
        HdrRemovingUa => "Удаление подмены User-Agent",
        UaRemovedTmpl => "Подмена UA удалена: {}",
        NoChangesToRemove => "В user.js нет наших изменений — удалять нечего.",
        RestartFirefoxNormalUa => "Перезапустите Firefox, чтобы вернуть обычный User-Agent.",
        HdrPunkbuster => "[2] PunkBuster",
        Bf3NotFound => "Battlefield 3 не найдена в ваших библиотеках Steam.",
        GameTmpl => "Игра: {}",
        PbFolderMissing => "Папка PunkBuster не найдена — античит не установлен?",
        NoPbClientDlls => "Клиентские DLL PunkBuster не найдены.",
        InstalledVersionTmpl => "Установленная версия клиента: v{}",
        CheckingOnline => "Проверяю текущую версию на сервере Even Balance…",
        OnlineUnreadable => "Не удалось получить онлайн-версию (нет сети / сервер недоступен) — сравниваю только локально.",
        NewerAvailableTmpl => "Онлайн доступна БОЛЕЕ НОВАЯ версия: v{} (у вас v{}). Обновите PunkBuster.",
        UpToDateTmpl => "Ваш PunkBuster АКТУАЛЕН (последняя онлайн-версия — v{}).",
        PbclLive => "Активный pbcl.dll = новейшая версия, античит работает.",
        PbclMismatch => "Активный pbcl.dll не совпадает с новейшей локальной DLL — переустановите через pbsetup.",
        HdrHowToUpdate => "Как обновить PunkBuster:",
        UpdateStep1 => "  1. Скачайте pbsetup.exe с evenbalance.com/downloads/pbsetup/pbsetup.zip → распакуйте в ~/Downloads/",
        UpdateStep2Tmpl => "  2. protontricks-launch --appid {} ~/Downloads/pbsetup.exe",
        UpdateStep3 => "  3. В pbsetup: «Add a game» → Battlefield 3 → «Add Game».",
        DoneLaunch => "Готово. Запустите BF3 из Steam (Играть), откройте Battlelog, нажмите «Join».",
        CliSetupHeader => "Battlefield 3 — настройка для Linux",
        WrapNoGameCmd => "bf3-linux-setup: после `--` не указана команда запуска игры",
        WrapLaunchFailedTmpl => "bf3-linux-setup: не удалось запустить игру ({}): {}",
        WrapBf3NotFound => "bf3-linux-setup: установка BF3 не найдена; пропускаю обновление PunkBuster.",
        WrapNoEvenBalance => "bf3-linux-setup: не удалось связаться с Even Balance; пропускаю обновление PunkBuster.",
        WrapRefreshedTmpl => "bf3-linux-setup: обновлён файл безопасности PunkBuster → {} (онлайн v{}); PunkBuster автоматически обновится при запуске.",
        WrapWriteFailedTmpl => "bf3-linux-setup: не удалось записать pbsec.htm: {}",
        WrapAlreadyCurrent => "bf3-linux-setup: PunkBuster уже актуален; обновление не требуется.",
        ReadFailedTmpl => "{}: ошибка чтения: {}",
        WriteFailedTmpl => "{}: ошибка записи: {}",
        DesktopName => "Battlefield 3 — настройка для Linux",
        DesktopComment => "Настройка Battlefield 3 для Battlelog на Linux: подмена ОС в Firefox + обновление PunkBuster",
        BtnInstallLauncher => "Добавить в меню приложений",
        HoverInstallLauncher => "Ставит ярлык и иконку в ~/.local (без root), чтобы инструмент появился в меню приложений",
        LauncherInstalledTmpl => "Добавлено в меню приложений: {}",
        LauncherFailedTmpl => "Не удалось добавить ярлык: {}",
        BtnUninstall => "Удалить…",
        HoverUninstall => "Убирает ярлык, иконку и (если он установлен туда) бинарь инструмента из ~/.local",
        ConfirmUninstallTitle => "Удаление",
        ConfirmUninstallBody => "Точно удалить ярлык из меню приложений, иконку и установленный бинарь? Внесённые исправления BF3 останутся на месте.",
        ChkUndoUaSpoof => "Заодно отменить подмену ОС в Firefox (User-Agent)",
        ConfirmDelete => "Удалить",
        ConfirmCancel => "Отмена",
        UninstallDone => "Удалено. Ярлык, иконка и установленный бинарь убраны.",
        UninstallFailedTmpl => "Не удалось удалить: {}",
    }
}

/// A static (non-parameterised) message in the active language.
pub fn t(k: Key) -> &'static str {
    pick(en(k), ru(k))
}

/// Both language strings for a key, as `(english, russian)`.
///
/// Unlike [`t`], which picks one language, this exposes both at once — needed
/// where a single artefact must carry every locale simultaneously, e.g. a
/// freedesktop `.desktop` file that lists `Name=`/`Name[ru]=` side by side.
pub fn both(k: Key) -> (&'static str, &'static str) {
    (en(k), ru(k))
}

/// A parameterised message: fills each `{}` placeholder in the active-language
/// template, in order, from `args`. Extra placeholders or args are ignored.
pub fn tf(k: Key, args: &[&str]) -> String {
    let template = t(k);
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    let mut it = args.iter();
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        if let Some(a) = it.next() {
            out.push_str(a);
        }
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ru_locales_select_russian() {
        for s in ["ru", "ru_RU", "ru_RU.UTF-8", "RU.utf8", "ru-RU", "ru@petr1708", "Ru_Ua"] {
            assert_eq!(lang_from_locale(s), Lang::Ru, "{s} should be Russian");
        }
    }

    #[test]
    fn en_and_other_locales_select_english() {
        for s in ["en", "en_US", "en_US.UTF-8", "en_GB", "C", "POSIX"] {
            assert_eq!(lang_from_locale(s), Lang::En, "{s} should be English");
        }
    }

    #[test]
    fn other_languages_select_english() {
        for s in ["de_DE.UTF-8", "fr_FR", "es_ES", "uk_UA", "zh_CN", "ja_JP"] {
            assert_eq!(lang_from_locale(s), Lang::En, "{s} should fall back to English");
        }
    }

    #[test]
    fn empty_and_garbage_select_english() {
        for s in ["", "   ", "rue", "russ", "rustic", "run"] {
            assert_eq!(lang_from_locale(s), Lang::En, "{s:?} should be English");
        }
    }

    #[test]
    fn tf_fills_placeholders_in_order() {
        // Uses a known two-placeholder template; language is process-detected,
        // so assert on structure that holds in either language.
        let s = tf(Key::NewerAvailableTmpl, &["2400", "2352"]);
        assert!(s.contains("2400"));
        assert!(s.contains("2352"));
        assert!(!s.contains("{}"));
    }

    #[test]
    fn tf_leaves_no_placeholder_when_args_supplied() {
        assert_eq!(tf(Key::GameTmpl, &["/games/bf3"]).contains("{}"), false);
        assert!(tf(Key::GameTmpl, &["/games/bf3"]).contains("/games/bf3"));
    }
}
