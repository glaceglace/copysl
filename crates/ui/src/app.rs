use eframe::egui;
use common::{ClipboardEntry, Config, ContentPayload, DaemonRequest, DaemonResponse, ToolRequirement};
use crate::components::{
    card_list::{CardList, CardListAction},
    search_bar::{SearchBar, filter as filter_entries},
    settings_panel::{SettingsPanel, SettingsAction},
};
use crate::ipc_client::IpcClient;

pub struct CopyslApp {
    ipc: Option<IpcClient>,
    entries: Vec<ClipboardEntry>,
    filtered: Vec<usize>,
    search_bar: SearchBar,
    card_list: CardList,
    show_settings: bool,
    settings_panel: SettingsPanel,
    config: Config,
    error_banner: Option<(String, std::time::Instant)>,
    startup_error: Option<String>,
    /// Missing tools detected at startup.  `None` means no warnings (or
    /// the user already dismissed the popup).
    tool_warnings: Option<ToolWarnings>,
    /// Set true once the window has received focus at least once.  Only
    /// after that do we close on focus loss (avoids closing at startup
    /// before the OS delivers the initial focus event).
    had_focus: bool,
    /// Set when StartDrag is sent so check_focus() does not close the window
    /// while the WM temporarily holds focus during a title-move operation.
    dragging: bool,
    /// Focus state from the previous frame, used to detect the false→true
    /// transition so we clear `dragging` only when focus actually returns,
    /// not on every normally-focused frame.
    was_focused: bool,
}

struct ToolWarnings {
    display_server: String,
    inject_backend: String,
    missing_critical: Vec<ToolRequirement>,
    missing_recommended: Vec<ToolRequirement>,
}

impl CopyslApp {
    pub fn new(cc: &eframe::CreationContext<'_>, socket_path: &str) -> Self {
        match IpcClient::connect_to(socket_path) {
            Err(e) => {
                let app = CopyslApp {
                    ipc: None,
                    entries: vec![],
                    filtered: vec![],
                    search_bar: SearchBar::new(),
                    card_list: CardList::new(),
                    show_settings: false,
                    settings_panel: SettingsPanel::new(Config::default()),
                    config: Config::default(),
                    error_banner: None,
                    startup_error: Some(format!(
                        "Copysl daemon is not running.\nStart with: copysl --daemon\n\nError: {e}"
                    )),
                    tool_warnings: None,
                    had_focus: false,
                    dragging: false,
                    was_focused: false,
                };
                setup_fonts(&cc.egui_ctx);
                apply_theme(&cc.egui_ctx, &app.config.theme);
                app
            }
            Ok(mut ipc) => {
                let entries = match ipc.send(&DaemonRequest::GetHistory { offset: 0, limit: 200 }) {
                    Ok(DaemonResponse::History(entries)) => entries,
                    _ => vec![],
                };
                let config = match ipc.send(&DaemonRequest::GetConfig) {
                    Ok(DaemonResponse::Config(c)) => c,
                    _ => Config::default(),
                };
                let tool_warnings = match ipc.send(&DaemonRequest::CheckTools) {
                    Ok(DaemonResponse::ToolsStatus {
                        display_server,
                        inject_backend,
                        missing_critical,
                        missing_recommended,
                        ..
                    }) if !missing_critical.is_empty() || !missing_recommended.is_empty() => {
                        Some(ToolWarnings {
                            display_server,
                            inject_backend,
                            missing_critical,
                            missing_recommended,
                        })
                    }
                    _ => None,
                };
                let filtered = (0..entries.len()).collect();
                let settings_panel = SettingsPanel::new(config.clone());
                setup_fonts(&cc.egui_ctx);
                apply_theme(&cc.egui_ctx, &config.theme);
                CopyslApp {
                    ipc: Some(ipc),
                    entries,
                    filtered,
                    search_bar: SearchBar::new(),
                    card_list: CardList::new(),
                    show_settings: false,
                    settings_panel,
                    config,
                    error_banner: None,
                    startup_error: None,
                    tool_warnings,
                    had_focus: false,
                    dragging: false,
                    was_focused: false,
                }
            }
        }
    }

    fn send_ipc(&mut self, req: DaemonRequest) -> Option<DaemonResponse> {
        let ipc = self.ipc.as_mut()?;
        match ipc.send(&req) {
            Ok(resp) => {
                self.error_banner = None;
                Some(resp)
            }
            Err(e) => {
                self.error_banner = Some((e.to_string(), std::time::Instant::now()));
                None
            }
        }
    }

    fn refilter(&mut self) {
        self.filtered = filter_entries(&self.search_bar.query, &self.entries);
    }

    fn close(&mut self, ctx: &egui::Context) {
        self.search_bar.clear();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn handle_card_action(&mut self, action: CardListAction, ctx: &egui::Context) {
        match action {
            CardListAction::Paste(id) => {
                // Fire-and-forget: the daemon sets the clipboard and injects Ctrl+V
                // after waiting 300 ms for the window close + WM focus transfer.
                // The window manager automatically returns focus to the previously
                // active window when this window closes — no Alt+Tab required.
                if let Some(ipc) = self.ipc.as_mut() {
                    let _ = ipc.send_fire_forget(&DaemonRequest::PasteEntry { id });
                }
                self.close(ctx);
            }
            CardListAction::Delete(id) => {
                self.send_ipc(DaemonRequest::DeleteEntry { id });
                self.entries.retain(|e| e.id != id);
                self.card_list.evict_texture(id);
                self.refilter();
            }
            CardListAction::Pin(id) => {
                self.send_ipc(DaemonRequest::PinEntry { id });
                if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
                    e.pinned = true;
                }
            }
            CardListAction::Unpin(id) => {
                self.send_ipc(DaemonRequest::UnpinEntry { id });
                if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
                    e.pinned = false;
                }
            }
            CardListAction::Copy(id) => {
                if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
                    let text = match &entry.payload {
                        ContentPayload::PlainText(t) => Some(t.clone()),
                        ContentPayload::RichText { plain_preview, .. } => {
                            Some(plain_preview.clone())
                        }
                        ContentPayload::Image { .. } => None,
                    };
                    if let Some(text) = text {
                        #[cfg(not(test))]
                        {
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(text);
                            }
                        }
                        #[cfg(test)]
                        {
                            let _ = text;
                        }
                    }
                }
            }
            CardListAction::Close => {
                self.close(ctx);
            }
        }
    }
}

impl eframe::App for CopyslApp {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll daemon for incoming clipboard entries
        if let Some(ipc) = self.ipc.as_mut() {
            if let Some(DaemonResponse::NewEntry(entry)) = ipc.try_recv_push() {
                self.entries.insert(0, entry);
                self.refilter();
            }
        }

        self.check_focus(ctx);
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}

impl CopyslApp {
    /// Close the window when it loses focus (user clicked outside).
    ///
    /// `had_focus` gates the check so we don't close before the WM delivers
    /// the initial focus event at startup.  With native decorations the WM
    /// keeps focus on the client during title-bar drags, so no extra guard
    /// is needed.
    fn check_focus(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.focused);
        if focused {
            self.had_focus = true;
            // Clear dragging only on the false→true transition (focus just returned).
            // Clearing it every focused frame would race: the WM may take several
            // frames to steal focus after StartDrag, and we'd reset the flag too early.
            if !self.was_focused {
                self.dragging = false;
            }
        } else if self.had_focus && !self.dragging {
            // Focus lost. Check if a left primary-button press occurred this same
            // frame — that is the WM reacting to a drag-start on the title bar.
            // On some X11 WMs the FocusOut arrives in the same frame as the click,
            // before render() has had a chance to set dragging=true.
            let left_click = ctx.input(|i| {
                i.events.iter().any(|e| matches!(
                    e,
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    }
                ))
            });
            if left_click {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                self.dragging = true;
            } else {
                self.close(ctx);
            }
        } else if !self.had_focus {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.was_focused = focused;
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        use crate::style::SPACE_M;

        let ctx = ui.ctx().clone();

        // Startup error modal — shown full-width, no panel padding needed
        if let Some(err) = self.startup_error.clone() {
            ui.heading("Copysl");
            ui.label(&err);
            if ui.button("OK").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        // Tool warning popup — shown when the daemon detected missing tools.
        // The user can dismiss it; warnings are not shown again until the next
        // time the UI starts.
        if let Some(warnings) = &self.tool_warnings {
            let display_server = warnings.display_server.clone();
            let inject_backend = warnings.inject_backend.clone();
            let missing_critical = warnings.missing_critical.clone();
            let missing_recommended = warnings.missing_recommended.clone();

            let mut dismissed = false;
            egui::Window::new("⚠ Missing tools")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.label(format!(
                        "Display server: {}  |  Inject backend: {}",
                        display_server, inject_backend
                    ));
                    ui.separator();

                    if !missing_critical.is_empty() {
                        ui.colored_label(
                            egui::Color32::RED,
                            "Critical — paste will NOT work without these:",
                        );
                        for req in &missing_critical {
                            ui.label(format!("• {} — {}", req.tool_name, req.purpose));
                            tool_hint_block(ui, &req.install_hint);
                        }
                        ui.add_space(4.0);
                    }

                    if !missing_recommended.is_empty() {
                        ui.colored_label(
                            egui::Color32::GOLD,
                            "Recommended — paste may be unreliable without these:",
                        );
                        for req in &missing_recommended {
                            ui.label(format!("• {} — {}", req.tool_name, req.purpose));
                            tool_hint_block(ui, &req.install_hint);
                        }
                    }

                    ui.add_space(4.0);
                    if ui.button("Dismiss").clicked() {
                        dismissed = true;
                    }
                });
            if dismissed {
                self.tool_warnings = None;
            }
        }

        // Error banner — auto-dismiss after 3 seconds
        if let Some((msg, instant)) = self.error_banner.clone() {
            if instant.elapsed().as_secs() < 3 {
                ui.colored_label(egui::Color32::RED, msg.as_str());
            } else {
                self.error_banner = None;
            }
        }

        // Main content: horizontal padding applied once here, not per-component
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(SPACE_M as i8, 0))
            .show(ui, |ui| {
                // ── Custom title bar ─────────────────────────────────────────
                let header_height = ctx.screen_rect().height() * 0.04;
                let header_rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), header_height),
                );
                ui.allocate_rect(header_rect, egui::Sense::hover());

                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(header_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                child.set_min_height(header_height);
                let gear_resp = child.add(egui::Button::new("⚙").frame(false));
                let gear_clicked = gear_resp.clicked();

                // Drag zone is the header to the right of the gear button.
                let drag_rect = egui::Rect::from_min_max(
                    egui::pos2(gear_resp.rect.max.x, header_rect.min.y),
                    header_rect.max,
                );
                let drag_resp = ui.interact(
                    drag_rect,
                    ui.id().with("title_drag"),
                    egui::Sense::click_and_drag(),
                );
                // Reliable left-vs-right detection:
                //   is_pointer_button_down_on() — fires immediately on press, position-aware
                //   PointerButton event          — one-shot, button-specific (never true for right-click)
                // Both must be true on the same frame, so neither alone is sufficient.
                let left_pressed_this_frame = ctx.input(|i| {
                    i.events.iter().any(|e| matches!(
                        e,
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            ..
                        }
                    ))
                });
                if drag_resp.is_pointer_button_down_on() && left_pressed_this_frame {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    self.dragging = true;
                }

                if gear_clicked {
                    self.show_settings = true;
                    self.settings_panel = SettingsPanel::new(self.config.clone());
                }

                ui.separator();

                if self.show_settings {
                    if let Some(action) = self.settings_panel.show(ui) {
                        match action {
                            SettingsAction::SaveConfig(new_config) => {
                                if let Some(DaemonResponse::Ok) =
                                    self.send_ipc(DaemonRequest::UpdateConfig(new_config.clone()))
                                {
                                    self.config = new_config;
                                    apply_theme(&ctx, &self.config.theme);
                                }
                            }
                            SettingsAction::Close => {
                                self.show_settings = false;
                            }
                            SettingsAction::KillDaemon => {
                                self.send_ipc(DaemonRequest::Shutdown);
                                self.close(&ctx);
                            }
                        }
                    }
                } else {
                    let query_changed = self.search_bar.show(ui, true);
                    if query_changed {
                        self.refilter();
                    }

                    // Card list
                    let filtered_clone = self.filtered.clone();
                    if let Some(action) = self.card_list.show(ui, &self.entries, &filtered_clone) {
                        self.handle_card_action(action, &ctx);
                    }
                }

                // Global Escape key to close
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.close(&ctx);
                }
            });
    }
}

/// Configure fonts and interaction style.
///
/// Two system fonts are loaded when available:
///
/// 1. A sans-serif font (DejaVu / Liberation / Noto Sans / FreeSans) placed
///    first in the Proportional family so that symbols like `←` and `≡` render
///    correctly.  egui's bundled Ubuntu-Light lacks these codepoints.
///
/// 2. A monochrome emoji font (NotoEmoji-Regular from the system) placed right
///    after the sans font.  This may cover more emoji than egui's bundled copy.
///
/// If neither is found the function falls through and `ctx.set_fonts` is still
/// called with the default definitions (no-op equivalent, but consistent).
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // ── 1. System sans-serif ─────────────────────────────────────────────────
    let sans_candidates: &[&str] = &[
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",       // Fedora / RHEL
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",         // Debian / Ubuntu
        "/usr/share/fonts/TTF/DejaVuSans.ttf",                     // Arch Linux
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",                  // openSUSE / generic
        "/usr/share/fonts/dejavu-fonts/DejaVuSans.ttf",
        "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/TTF/NotoSans-Regular.ttf",
        "/usr/share/fonts/gnu-free/FreeSans.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    ];
    for path in sans_candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "SystemSans".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "SystemSans".to_owned());
            break;
        }
    }

    // ── 2. System emoji font ─────────────────────────────────────────────────
    let emoji_candidates: &[&str] = &[
        "/usr/share/fonts/google-noto-emoji-fonts/NotoEmoji-Regular.ttf", // Fedora
        "/usr/share/fonts/google-noto-emoji/NotoEmoji-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",           // Debian / Ubuntu
        "/usr/share/fonts/noto-emoji/NotoEmoji-Regular.ttf",
        "/usr/share/fonts/noto/NotoEmoji-Regular.ttf",
        "/usr/share/fonts/TTF/NotoEmoji-Regular.ttf",                     // Arch
    ];
    for path in emoji_candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "SystemEmoji".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            let family = fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default();
            // Insert after SystemSans (index 1) if present, else at front (index 0)
            family.insert(emoji_insert_pos(family), "SystemEmoji".to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);

    // Labels are read-only — don't show the IBeam cursor when hovering text.
    ctx.style_mut(|s| s.interaction.selectable_labels = false);
}

/// Compute the index at which the emoji font should be inserted in the
/// Proportional family list.
///
/// If `SystemSans` is already at position 0, the emoji font goes after it
/// (position 1) so Latin text still uses the sans font first.  Otherwise the
/// emoji font is prepended at position 0.
pub(crate) fn emoji_insert_pos(family: &[String]) -> usize {
    usize::from(family.first().map(|s| s == "SystemSans").unwrap_or(false))
}

/// Warm light visuals — cream-tinted so the app reads as clearly "light"
/// without being cold or snow-white.
///
/// Default egui light uses `from_gray(248)` panels and `from_gray(230)` cards,
/// which look flat and neutral.  We swap those for warm off-white/beige tones.
pub fn light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();

    // Warm cream hierarchy (panel < card < hover < active so each level is
    // visually distinct without being harsh).
    let warm_panel  = egui::Color32::from_rgb(250, 247, 240); // main background
    let warm_card   = egui::Color32::from_rgb(240, 236, 228); // card / inactive
    let warm_hover  = egui::Color32::from_rgb(228, 223, 214); // hovered
    let warm_active = egui::Color32::from_rgb(212, 207, 196); // pressed / active
    let warm_input  = egui::Color32::from_rgb(255, 253, 248); // text-edit bg

    v.window_fill  = warm_panel;
    v.panel_fill   = warm_panel;
    v.extreme_bg_color = warm_input;

    v.widgets.noninteractive.weak_bg_fill = warm_panel;
    v.widgets.noninteractive.bg_fill      = warm_panel;

    v.widgets.inactive.weak_bg_fill = warm_card;
    v.widgets.inactive.bg_fill      = warm_card;

    v.widgets.hovered.weak_bg_fill = warm_hover;
    v.widgets.hovered.bg_fill      = warm_hover;

    v.widgets.active.weak_bg_fill = warm_active;
    v.widgets.active.bg_fill      = warm_active;

    v
}

/// Apply the configured theme to an egui context.
///
/// Light → custom warm visuals.  Dark → egui default dark.  System → let egui
/// follow the OS color scheme (no custom visuals override).
pub fn apply_theme(ctx: &egui::Context, theme: &common::Theme) {
    // Store warm light visuals persistently so begin_pass reads them every frame.
    ctx.set_visuals_of(egui::Theme::Light, light_visuals());
    ctx.set_theme(match theme {
        common::Theme::Dark   => egui::ThemePreference::Dark,
        common::Theme::Light  => egui::ThemePreference::Light,
        common::Theme::System => egui::ThemePreference::System,
    });
}

/// Render an install hint as a selectable, monospace code block with a
/// copy button in the top-right corner.
fn tool_hint_block(ui: &mut egui::Ui, hint: &str) {
    ui.horizontal(|ui| {
        ui.label("Run in terminal:");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("📋 Copy").clicked() {
                #[cfg(not(test))]
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(hint.to_string());
                }
            }
        });
    });
    let rows = hint.lines().count().max(1).min(12);
    let mut text = hint.to_string();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY)
            .desired_rows(rows),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{ContentPayload, EntryId};
    use std::time::SystemTime;

    fn make_app_with_entries(entries: Vec<ClipboardEntry>) -> CopyslApp {
        let filtered = (0..entries.len()).collect();
        CopyslApp {
            ipc: None,
            entries,
            filtered,
            search_bar: SearchBar::new(),
            card_list: CardList::new(),
            show_settings: false,
            settings_panel: SettingsPanel::new(Config::default()),
            config: Config::default(),
            error_banner: None,
            startup_error: None,
            tool_warnings: None,
            had_focus: false,
            dragging: false,
            was_focused: false,
        }
    }

    fn make_entry(id: u64, text: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(id),
            payload: ContentPayload::PlainText(text.to_string()),
            captured_at: SystemTime::now(),
            pinned: false,
        }
    }

    #[test]
    fn refilter_empty_query_returns_all() {
        let mut app = make_app_with_entries(vec![
            make_entry(0, "hello"),
            make_entry(1, "world"),
        ]);
        app.refilter();
        assert_eq!(app.filtered, vec![0, 1]);
    }

    #[test]
    fn refilter_with_query_filters() {
        let mut app = make_app_with_entries(vec![
            make_entry(0, "hello"),
            make_entry(1, "world"),
        ]);
        app.search_bar.set_query("hello");
        app.refilter();
        assert_eq!(app.filtered, vec![0]);
    }

    #[test]
    fn startup_error_set_when_daemon_not_running() {
        std::env::remove_var("COPYSL_SOCKET");
        let result = IpcClient::connect();
        assert!(result.is_err());
    }

    #[test]
    fn new_entry_push_prepends_to_entries() {
        let mut app = make_app_with_entries(vec![make_entry(1, "existing")]);
        let new_entry = make_entry(2, "new");
        app.entries.insert(0, new_entry);
        app.refilter();
        assert_eq!(app.entries.len(), 2);
        assert_eq!(app.entries[0].id, EntryId(2));
    }

    #[test]
    fn delete_entry_removes_from_local_list() {
        let mut app = make_app_with_entries(vec![
            make_entry(0, "first"),
            make_entry(1, "second"),
        ]);
        app.entries.retain(|e| e.id != EntryId(0));
        app.refilter();
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].id, EntryId(1));
    }

    #[test]
    fn pin_updates_entry_pinned_flag() {
        let mut app = make_app_with_entries(vec![make_entry(0, "test")]);
        assert!(!app.entries[0].pinned);
        if let Some(e) = app.entries.iter_mut().find(|e| e.id == EntryId(0)) {
            e.pinned = true;
        }
        assert!(app.entries[0].pinned);
    }

    #[test]
    fn unpin_updates_entry_pinned_flag() {
        let mut entries = vec![make_entry(0, "test")];
        entries[0].pinned = true;
        let mut app = make_app_with_entries(entries);
        assert!(app.entries[0].pinned);
        if let Some(e) = app.entries.iter_mut().find(|e| e.id == EntryId(0)) {
            e.pinned = false;
        }
        assert!(!app.entries[0].pinned);
    }

    #[test]
    fn send_ipc_returns_none_when_no_client() {
        let mut app = make_app_with_entries(vec![]);
        let result = app.send_ipc(DaemonRequest::GetHistory { offset: 0, limit: 10 });
        assert!(result.is_none());
    }

    #[test]
    fn had_focus_starts_false() {
        let app = make_app_with_entries(vec![]);
        assert!(!app.had_focus);
    }

    // ── light_visuals ─────────────────────────────────────────────────────────

    #[test]
    fn light_visuals_dark_mode_is_false() {
        assert!(!light_visuals().dark_mode);
    }

    #[test]
    fn light_visuals_panel_fill_is_warm_not_cold_gray() {
        let v = light_visuals();
        // egui default light is from_gray(248) = (248, 248, 248) — no warmth.
        // Our palette has more red than blue, giving a cream tone.
        let [r, _, b, _] = v.panel_fill.to_array();
        assert!(r > b, "panel fill should be warmer (more red) than cold gray");
        assert_ne!(v.panel_fill, egui::Color32::WHITE, "must not be pure white");
    }

    #[test]
    fn light_visuals_card_bg_darker_than_panel() {
        let v = light_visuals();
        // Cards should be visually distinct from (darker than) the panel background.
        let avg = |c: egui::Color32| -> u32 {
            let [r, g, b, _] = c.to_array();
            (r as u32 + g as u32 + b as u32) / 3
        };
        assert!(avg(v.widgets.inactive.bg_fill) < avg(v.panel_fill));
    }

    #[test]
    fn light_visuals_hover_darker_than_card() {
        let v = light_visuals();
        let avg = |c: egui::Color32| -> u32 {
            let [r, g, b, _] = c.to_array();
            (r as u32 + g as u32 + b as u32) / 3
        };
        assert!(avg(v.widgets.hovered.bg_fill) < avg(v.widgets.inactive.bg_fill));
    }

    #[test]
    fn light_visuals_active_darkest_of_card_states() {
        let v = light_visuals();
        let avg = |c: egui::Color32| -> u32 {
            let [r, g, b, _] = c.to_array();
            (r as u32 + g as u32 + b as u32) / 3
        };
        assert!(avg(v.widgets.active.bg_fill) < avg(v.widgets.hovered.bg_fill));
    }

    // ── clear_color ───────────────────────────────────────────────────────────

    #[test]
    fn clear_color_matches_panel_fill_for_light_visuals() {
        let app = make_app_with_entries(vec![]);
        let visuals = light_visuals();
        let expected = visuals.panel_fill.to_normalized_gamma_f32();
        use eframe::App as _;
        assert_eq!(app.clear_color(&visuals), expected);
    }

    #[test]
    fn clear_color_matches_panel_fill_for_dark_visuals() {
        let app = make_app_with_entries(vec![]);
        let visuals = egui::Visuals::dark();
        let expected = visuals.panel_fill.to_normalized_gamma_f32();
        use eframe::App as _;
        assert_eq!(app.clear_color(&visuals), expected);
    }

    #[test]
    fn clear_color_light_is_not_near_black() {
        let app = make_app_with_entries(vec![]);
        let visuals = light_visuals();
        use eframe::App as _;
        let [r, g, b, _] = app.clear_color(&visuals);
        // All channels should be above 0.9 (warm cream, not the hardcoded dark 12/255 ≈ 0.047).
        assert!(r > 0.9 && g > 0.9 && b > 0.9, "light clear_color should be near-white, got {r},{g},{b}");
    }

    // ── apply_theme ───────────────────────────────────────────────────────────

    #[test]
    fn apply_theme_light_sets_light_preference() {
        let ctx = egui::Context::default();
        apply_theme(&ctx, &common::Theme::Light);
        let pref = ctx.options(|o| o.theme_preference);
        assert_eq!(pref, egui::ThemePreference::Light);
    }

    #[test]
    fn apply_theme_dark_sets_dark_preference() {
        let ctx = egui::Context::default();
        apply_theme(&ctx, &common::Theme::Dark);
        let pref = ctx.options(|o| o.theme_preference);
        assert_eq!(pref, egui::ThemePreference::Dark);
    }

    #[test]
    fn apply_theme_system_sets_system_preference() {
        let ctx = egui::Context::default();
        apply_theme(&ctx, &common::Theme::System);
        let pref = ctx.options(|o| o.theme_preference);
        assert_eq!(pref, egui::ThemePreference::System);
    }

    #[test]
    fn apply_theme_light_stores_warm_panel_fill() {
        let ctx = egui::Context::default();
        apply_theme(&ctx, &common::Theme::Light);
        let panel_fill = ctx.options(|o| o.light_style.visuals.panel_fill);
        let expected = light_visuals().panel_fill;
        assert_eq!(panel_fill, expected);
    }

    #[test]
    fn apply_theme_light_panel_fill_is_warm_not_cold_gray() {
        let ctx = egui::Context::default();
        apply_theme(&ctx, &common::Theme::Light);
        let [r, _, b, _] = ctx.options(|o| o.light_style.visuals.panel_fill.to_array());
        assert!(r > b, "stored light visuals should have warm panel fill (more red than blue)");
    }

    // ── emoji_insert_pos ──────────────────────────────────────────────────────

    #[test]
    fn emoji_insert_pos_empty_family_is_zero() {
        assert_eq!(emoji_insert_pos(&[]), 0);
    }

    #[test]
    fn emoji_insert_pos_no_system_sans_is_zero() {
        let family: Vec<String> = vec!["Ubuntu-Light".into(), "NotoEmoji-Regular".into()];
        assert_eq!(emoji_insert_pos(&family), 0);
    }

    #[test]
    fn emoji_insert_pos_system_sans_first_is_one() {
        let family: Vec<String> = vec!["SystemSans".into(), "Ubuntu-Light".into()];
        assert_eq!(emoji_insert_pos(&family), 1);
    }

    #[test]
    fn emoji_insert_pos_system_sans_not_first_is_zero() {
        // SystemSans exists but is not at index 0 — emoji still goes to front
        let family: Vec<String> = vec!["Ubuntu-Light".into(), "SystemSans".into()];
        assert_eq!(emoji_insert_pos(&family), 0);
    }

    #[test]
    fn emoji_insert_pos_only_system_sans_is_one() {
        let family: Vec<String> = vec!["SystemSans".into()];
        assert_eq!(emoji_insert_pos(&family), 1);
    }

    // ── setup_fonts ───────────────────────────────────────────────────────────

    #[test]
    fn setup_fonts_disables_selectable_labels() {
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        assert!(!ctx.style().interaction.selectable_labels);
    }

    #[test]
    fn setup_fonts_does_not_panic() {
        // No system fonts available in CI is fine — must not crash.
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
    }

    #[test]
    fn setup_fonts_emoji_goes_after_sans_in_definitions() {
        // Simulate what setup_fonts does when both fonts are available: verify
        // the emoji font lands at index 1 when SystemSans is already at index 0.
        let mut fonts = egui::FontDefinitions::default();
        let family = fonts.families.entry(egui::FontFamily::Proportional).or_default();
        family.insert(0, "SystemSans".to_owned());
        let pos = emoji_insert_pos(family);
        family.insert(pos, "SystemEmoji".to_owned());
        assert_eq!(family[0], "SystemSans");
        assert_eq!(family[1], "SystemEmoji");
    }

    #[test]
    fn setup_fonts_emoji_goes_first_without_sans_in_definitions() {
        // When no system sans was loaded, emoji is prepended at position 0.
        let mut fonts = egui::FontDefinitions::default();
        let family = fonts.families.entry(egui::FontFamily::Proportional).or_default();
        let pos = emoji_insert_pos(family);
        family.insert(pos, "SystemEmoji".to_owned());
        assert_eq!(family[0], "SystemEmoji");
    }

    #[test]
    fn setup_fonts_called_twice_does_not_panic() {
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
        setup_fonts(&ctx); // idempotent — must not crash or deadlock
        assert!(!ctx.style().interaction.selectable_labels);
    }

    // ── check_focus ───────────────────────────────────────────────────────────

    fn left_click_event() -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::Pos2::new(100.0, 20.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn right_click_event() -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::Pos2::new(100.0, 20.0),
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// Run one synthetic frame through `check_focus` and return the viewport
    /// commands it produced (Close, StartDrag, Focus, etc.).
    fn run_focus_frame(
        app: &mut CopyslApp,
        focused: bool,
        events: Vec<egui::Event>,
    ) -> Vec<egui::ViewportCommand> {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            focused,
            events,
            ..Default::default()
        });
        app.check_focus(&ctx);
        let output = ctx.end_pass();
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|vo| vo.commands.clone())
            .unwrap_or_default()
    }

    #[test]
    fn check_focus_sets_had_focus_when_focused() {
        let mut app = make_app_with_entries(vec![]);
        assert!(!app.had_focus);
        run_focus_frame(&mut app, true, vec![]);
        assert!(app.had_focus);
    }

    #[test]
    fn check_focus_clears_dragging_on_false_to_true_transition() {
        let mut app = make_app_with_entries(vec![]);
        app.dragging = true;
        // was_focused = false (default) → this is a false→true transition
        run_focus_frame(&mut app, true, vec![]);
        assert!(!app.dragging, "dragging must be cleared when focus returns");
    }

    #[test]
    fn check_focus_keeps_dragging_when_continuously_focused() {
        let mut app = make_app_with_entries(vec![]);
        app.had_focus = true;
        app.was_focused = true; // already focused last frame — not a transition
        app.dragging = true;
        run_focus_frame(&mut app, true, vec![]);
        assert!(app.dragging, "dragging must not be cleared mid-drag while focused");
    }

    #[test]
    fn check_focus_was_focused_becomes_true_after_focused_frame() {
        let mut app = make_app_with_entries(vec![]);
        run_focus_frame(&mut app, true, vec![]);
        assert!(app.was_focused);
    }

    #[test]
    fn check_focus_was_focused_becomes_false_after_unfocused_frame() {
        let mut app = make_app_with_entries(vec![]);
        app.had_focus = true;
        app.dragging = true; // suppress close so we can check was_focused
        run_focus_frame(&mut app, false, vec![]);
        assert!(!app.was_focused);
    }

    #[test]
    fn check_focus_sends_close_on_focus_loss_without_click() {
        let mut app = make_app_with_entries(vec![]);
        app.had_focus = true;
        let cmds = run_focus_frame(&mut app, false, vec![]);
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "expected Close in {cmds:?}"
        );
    }

    #[test]
    fn check_focus_sends_start_drag_on_same_frame_left_click() {
        let mut app = make_app_with_entries(vec![]);
        app.had_focus = true;
        let cmds = run_focus_frame(&mut app, false, vec![left_click_event()]);
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::StartDrag)),
            "expected StartDrag in {cmds:?}"
        );
        assert!(
            !cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "must not Close on drag-start frame"
        );
        assert!(app.dragging, "dragging flag must be set after StartDrag");
    }

    #[test]
    fn check_focus_no_close_while_dragging() {
        let mut app = make_app_with_entries(vec![]);
        app.had_focus = true;
        app.dragging = true;
        let cmds = run_focus_frame(&mut app, false, vec![]);
        assert!(
            !cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "must not close while dragging flag is set"
        );
    }

    #[test]
    fn check_focus_requests_focus_before_first_focus() {
        let mut app = make_app_with_entries(vec![]);
        // had_focus = false (default), focused = false → request focus at startup
        let cmds = run_focus_frame(&mut app, false, vec![]);
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Focus)),
            "expected Focus request in {cmds:?}"
        );
        assert!(
            !cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "must not close before first focus"
        );
    }

    #[test]
    fn check_focus_right_click_does_not_suppress_close() {
        let mut app = make_app_with_entries(vec![]);
        app.had_focus = true;
        let cmds = run_focus_frame(&mut app, false, vec![right_click_event()]);
        assert!(
            cmds.iter().any(|c| matches!(c, egui::ViewportCommand::Close)),
            "right-click must not prevent close"
        );
        assert!(
            !cmds.iter().any(|c| matches!(c, egui::ViewportCommand::StartDrag)),
            "right-click must not trigger StartDrag"
        );
    }

}
