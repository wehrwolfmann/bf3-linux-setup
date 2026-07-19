//! egui/eframe front-end. A single static binary — no system GTK/Qt — so it
//! runs on immutable SteamOS desktop mode. Visual style mirrors the reference
//! Python's Steam-dark skin.

use crate::actions::{self, Level, Log, Status};
use crate::consts::BF3_APPID;
use crate::i18n::{t, tf, Key};
use crate::{launcher, steam_launch, uninstall};
use eframe::egui::{self, Color32, RichText};

// ── Steam-dark palette (ported from the Python `ST` dict) ──────────────
const BG: Color32 = Color32::from_rgb(0x1b, 0x28, 0x38);
const BG2: Color32 = Color32::from_rgb(0x16, 0x20, 0x2d);
const CARD: Color32 = Color32::from_rgb(0x2a, 0x40, 0x55);
const ACCENT: Color32 = Color32::from_rgb(0x66, 0xc0, 0xf4);
const GREEN: Color32 = Color32::from_rgb(0x5b, 0xa3, 0x2b);
const GREEN_HI: Color32 = Color32::from_rgb(0x6f, 0xc7, 0x33);
const TEXT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
const DIM: Color32 = Color32::from_rgb(0x9f, 0xb4, 0xc6);
const AMBER: Color32 = Color32::from_rgb(0xff, 0xc3, 0x00);
const RED: Color32 = Color32::from_rgb(0xd9, 0x4a, 0x4a);

fn level_color(level: Level) -> Color32 {
    match level {
        Level::Header => ACCENT,
        Level::Ok => GREEN_HI,
        Level::Warn => AMBER,
        Level::Info => DIM,
        Level::Plain => TEXT,
    }
}

#[derive(Default)]
pub struct Bf3App {
    lines: Vec<(Level, String)>,
    ua_status: Option<Status>,
    pb_status: Option<Status>,
    launch_note: String,
    /// The uninstall confirmation dialog is open.
    confirm_uninstall: bool,
    /// Checkbox in that dialog: also undo the Firefox OS spoof.
    undo_ua_on_uninstall: bool,
}

impl Bf3App {
    pub fn new() -> Self {
        let mut app = Self::default();
        app.push_plain(Level::Info, t(Key::GuiHint));
        app
    }

    fn push_plain(&mut self, level: Level, text: &str) {
        self.lines.push((level, text.to_string()));
    }

    fn absorb(&mut self, log: Log) {
        for line in log.lines {
            self.lines.push((line.level, line.text));
        }
    }

    fn run_fix(&mut self) {
        self.lines.clear();
        let mut log = Log::new();
        let (ua, pb) = actions::run_setup(&mut log);
        self.ua_status = Some(ua);
        self.pb_status = Some(pb);
        self.absorb(log);
    }

    fn run_undo(&mut self) {
        self.lines.clear();
        let mut log = Log::new();
        actions::do_ua_off(&mut log);
        self.ua_status = None;
        self.absorb(log);
    }

    fn install_wrapper(&mut self, ctx: &egui::Context) {
        let exe = steam_launch::current_exe_string();
        let opts = steam_launch::launch_options_string(&exe);
        ctx.copy_text(opts.clone());

        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let mut note = tf(Key::NoteCopiedTmpl, &[&opts]);
        if let Some(home) = home {
            let (updated, issues) = steam_launch::install_launch_options(&home, BF3_APPID, &opts);
            if !updated.is_empty() {
                note.push_str(t(Key::NoteWrittenAuto));
                for p in updated {
                    note.push_str(&format!("\n    {}", p.display()));
                }
            } else {
                note.push_str(t(Key::NoteCouldNotWrite));
                if let Some(first) = issues.first() {
                    note.push_str(&tf(Key::NoteReasonTmpl, &[first]));
                }
                note.push_str(t(Key::NotePasteManually));
            }
        }
        self.launch_note = note;
    }

    /// Install the app-menu launcher (`.desktop` + icon) under `~/.local`.
    fn install_launcher(&mut self) {
        let home = launcher::home_dir();
        let exe = std::path::PathBuf::from(launcher::current_exe_string());
        match launcher::install(&home, &exe) {
            Ok(paths) => self.push_plain(
                Level::Ok,
                &tf(Key::LauncherInstalledTmpl, &[&paths.desktop.display().to_string()]),
            ),
            Err(e) => self.push_plain(Level::Warn, &tf(Key::LauncherFailedTmpl, &[&e])),
        }
    }

    /// Run the uninstall the user confirmed in the dialog: remove the launcher,
    /// icon and (standard-path) binary, and optionally undo the Firefox spoof.
    fn run_uninstall(&mut self) {
        let home = uninstall::home_dir();
        let plan = uninstall::plan(&uninstall::Layout::from_home(&home));
        match uninstall::execute(&plan, &uninstall::RealExecutor) {
            Ok(()) => self.push_plain(Level::Ok, t(Key::UninstallDone)),
            Err(e) => self.push_plain(Level::Warn, &tf(Key::UninstallFailedTmpl, &[&e])),
        }
        if self.undo_ua_on_uninstall {
            let mut log = Log::new();
            actions::do_ua_off(&mut log);
            self.ua_status = None;
            self.absorb(log);
        }
    }

    /// The modal-ish confirmation window for uninstall («Точно удалить?»).
    /// Nothing is deleted until "Delete" is pressed here.
    fn uninstall_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_uninstall {
            return;
        }
        let mut open = true;
        let mut do_it = false;
        egui::Window::new(RichText::new(t(Key::ConfirmUninstallTitle)).color(ACCENT))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new(t(Key::ConfirmUninstallBody)).color(TEXT));
                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.undo_ua_on_uninstall,
                    RichText::new(t(Key::ChkUndoUaSpoof)).color(DIM),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let del = egui::Button::new(RichText::new(t(Key::ConfirmDelete)).color(TEXT))
                        .fill(RED);
                    if ui.add(del).clicked() {
                        do_it = true;
                    }
                    if ui.button(RichText::new(t(Key::ConfirmCancel)).color(TEXT)).clicked() {
                        self.confirm_uninstall = false;
                    }
                });
            });
        if !open {
            self.confirm_uninstall = false;
        }
        if do_it {
            self.confirm_uninstall = false;
            self.run_uninstall();
        }
    }
}

fn status_dot(ui: &mut egui::Ui, label: &str, status: Option<Status>) {
    let (color, glyph) = match status {
        Some(Status::Ok) => (GREEN_HI, "●"),
        Some(Status::Warn) => (RED, "●"),
        None => (DIM, "○"),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(glyph).color(color).size(16.0));
        ui.label(RichText::new(label).color(TEXT));
    });
}

impl eframe::App for Bf3App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = BG;
        visuals.extreme_bg_color = BG2;
        visuals.override_text_color = Some(TEXT);
        visuals.widgets.inactive.weak_bg_fill = CARD;
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x39, 0x58, 0x72);
        ctx.set_visuals(visuals);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new(t(Key::AppTitle)).color(ACCENT).size(20.0).strong());
            ui.label(RichText::new(t(Key::GuiSubtitle)).color(DIM));
            ui.separator();

            // Status dots.
            ui.horizontal(|ui| {
                status_dot(ui, t(Key::DotUa), self.ua_status);
                ui.add_space(24.0);
                status_dot(ui, t(Key::DotPunkbuster), self.pb_status);
            });
            ui.add_space(6.0);

            // Primary actions.
            ui.horizontal(|ui| {
                let fix = egui::Button::new(
                    RichText::new(t(Key::BtnFix)).color(TEXT).strong(),
                )
                .fill(GREEN)
                .min_size(egui::vec2(0.0, 34.0));
                if ui.add(fix).clicked() {
                    self.run_fix();
                }
                if ui.button(RichText::new(t(Key::BtnUndo)).color(TEXT)).clicked() {
                    self.run_undo();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new(t(Key::BtnInstallWrapper)).color(TEXT))
                    .on_hover_text(t(Key::HoverInstallWrapper))
                    .clicked()
                {
                    self.install_wrapper(ctx);
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new(t(Key::BtnInstallLauncher)).color(TEXT))
                    .on_hover_text(t(Key::HoverInstallLauncher))
                    .clicked()
                {
                    self.install_launcher();
                }
                if ui
                    .button(RichText::new(t(Key::BtnUninstall)).color(RED))
                    .on_hover_text(t(Key::HoverUninstall))
                    .clicked()
                {
                    self.undo_ua_on_uninstall = false;
                    self.confirm_uninstall = true;
                }
            });

            self.uninstall_dialog(ctx);

            if !self.launch_note.is_empty() {
                ui.add_space(4.0);
                egui::Frame::none().fill(BG2).inner_margin(8.0).show(ui, |ui| {
                    ui.label(RichText::new(&self.launch_note).color(ACCENT).monospace());
                });
            }

            ui.add_space(6.0);
            ui.separator();

            // Log.
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Frame::none().fill(BG2).inner_margin(8.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (level, text) in &self.lines {
                        let prefix = match level {
                            Level::Ok => "✔ ",
                            Level::Warn => "! ",
                            Level::Info => "· ",
                            _ => "",
                        };
                        let mut rt = RichText::new(format!("{prefix}{text}"))
                            .color(level_color(*level))
                            .monospace();
                        if *level == Level::Header {
                            rt = rt.strong();
                        }
                        ui.label(rt);
                    }
                });
            });
        });
    }
}

/// Launch the GUI event loop. Returns an error only if the window system is
/// unavailable (e.g. no display).
pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([600.0, 440.0])
            // Must equal launcher::APP_ID / .desktop StartupWMClass so the
            // desktop maps this window to the installed menu launcher.
            .with_app_id(launcher::APP_ID)
            .with_title(t(Key::AppTitle)),
        ..Default::default()
    };
    eframe::run_native(
        "bf3-linux-setup",
        options,
        Box::new(|_cc| Ok(Box::new(Bf3App::new()))),
    )
}
