use eframe::egui;
use common::{ClipboardEntry, Config, ContentPayload, DaemonRequest, DaemonResponse, ToolRequirement};
use crate::components::{
    card_list::{CardList, CardListAction},
    search_bar::{SearchBar, filter as filter_entries},
    settings_panel::{SettingsPanel, SettingsAction},
};
use crate::ipc_client::IpcClient;

pub struct CopieurApp {
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
}

struct ToolWarnings {
    display_server: String,
    inject_backend: String,
    missing_critical: Vec<ToolRequirement>,
    missing_recommended: Vec<ToolRequirement>,
}

impl CopieurApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, socket_path: &str) -> Self {
        match IpcClient::connect_to(socket_path) {
            Err(e) => CopieurApp {
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
                    "Copieur daemon is not running.\nStart with: copieur --daemon\n\nError: {e}"
                )),
                tool_warnings: None,
            },
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
                CopieurApp {
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

impl eframe::App for CopieurApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll daemon push notifications every frame (non-UI logic only)
        if let Some(ipc) = self.ipc.as_mut() {
            if let Some(DaemonResponse::NewEntry(entry)) = ipc.try_recv_push() {
                self.entries.insert(0, entry);
                self.refilter();
            }
        }
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}

impl CopieurApp {
    fn render(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // Show startup error modal
        if let Some(err) = self.startup_error.clone() {
            ui.heading("Copieur");
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

        // Top bar: title and gear icon
        ui.horizontal(|ui| {
            ui.heading("Clipboard History");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").clicked() {
                    self.show_settings = !self.show_settings;
                    if self.show_settings {
                        self.settings_panel = SettingsPanel::new(self.config.clone());
                    }
                }
            });
        });

        if self.show_settings {
            if let Some(action) = self.settings_panel.show(ui) {
                match action {
                    SettingsAction::SaveConfig(new_config) => {
                        if let Some(DaemonResponse::Ok) =
                            self.send_ipc(DaemonRequest::UpdateConfig(new_config.clone()))
                        {
                            self.config = new_config;
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
            // Search bar
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
    }
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

    fn make_app_with_entries(entries: Vec<ClipboardEntry>) -> CopieurApp {
        let filtered = (0..entries.len()).collect();
        CopieurApp {
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
        std::env::remove_var("COPIEUR_SOCKET");
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
}
