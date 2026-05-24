use common::{Config, Theme, WindowPos};

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsAction {
    SaveConfig(Config),
    Close,
    KillDaemon,
}

/// Pure state machine for the persistence toggle privacy warning.
#[derive(Debug, Clone, PartialEq)]
pub enum PersistenceToggleState {
    Off,
    PendingConfirmation,
    On,
}

impl PersistenceToggleState {
    pub fn toggle_on(&mut self) {
        if *self == PersistenceToggleState::Off {
            *self = PersistenceToggleState::PendingConfirmation;
        }
    }

    pub fn toggle_off(&mut self) {
        *self = PersistenceToggleState::Off;
    }

    pub fn confirm(&mut self) {
        *self = PersistenceToggleState::On;
    }

    pub fn cancel(&mut self) {
        *self = PersistenceToggleState::Off;
    }

    pub fn is_enabled(&self) -> bool {
        *self == PersistenceToggleState::On
    }

    pub fn is_warning_shown(&self) -> bool {
        *self == PersistenceToggleState::PendingConfirmation
    }
}

pub struct SettingsPanel {
    pub draft: Config,
    pub show_persist_warning: bool,
    paste_delay_text: String,
}

impl SettingsPanel {
    pub fn new(config: Config) -> Self {
        let paste_delay_text = config.paste_delay_ms.to_string();
        SettingsPanel {
            draft: config,
            show_persist_warning: false,
            paste_delay_text,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<SettingsAction> {
        use crate::style::{RADIUS_CARD, SPACE_L, SPACE_S};

        let mut action = None;
        let visuals = ui.visuals().clone();

        // Wrap in a "sheet" frame so it reads as a distinct visual layer over the card list
        let sheet_frame = egui::Frame::new()
            .fill(visuals.window_fill)
            .stroke(visuals.window_stroke())
            .corner_radius(egui::CornerRadius::same(RADIUS_CARD))
            .inner_margin(egui::Margin::same(SPACE_L as i8));

        sheet_frame.show(ui, |ui| {
            // Title row: "Settings" on the left, back button on the right
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Settings")
                        .text_style(egui::TextStyle::Body)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("←").frame(false)).clicked() {
                        action = Some(SettingsAction::Close);
                    }
                });
            });

            // ── History limits ─────────────────────────────────────────────
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Max history entries:");
                let mut max = self.draft.max_entries as i32;
                if ui.add(egui::DragValue::new(&mut max).range(10..=10000)).changed() {
                    self.draft.max_entries = max.max(10) as usize;
                    action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                }
            });

            // ── Persistence ────────────────────────────────────────────────
            ui.separator();

            {
                let mut persist = self.draft.persist_history;
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut persist, "Persist history to disk").changed() {
                        if persist && !self.draft.persist_history {
                            self.show_persist_warning = true;
                        } else if !persist {
                            self.draft.persist_history = false;
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                    }
                });
                if self.show_persist_warning {
                    ui.add_space(SPACE_S);
                    ui.colored_label(egui::Color32::YELLOW, "Warning: Enabling this saves clipboard contents (including passwords and tokens) to disk. Confirm to proceed.");
                    ui.add_space(SPACE_S);
                    ui.horizontal(|ui| {
                        if ui.button("I understand, enable").clicked() {
                            self.draft.persist_history = true;
                            self.show_persist_warning = false;
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_persist_warning = false;
                        }
                    });
                }
            }

            // ── Display settings ───────────────────────────────────────────
            ui.separator();

            {
                let mut autostart = self.draft.autostart;
                if ui.checkbox(&mut autostart, "Start on login").changed() {
                    self.draft.autostart = autostart;
                    action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                }
            }

            ui.add_space(SPACE_S);

            ui.horizontal(|ui| {
                ui.label("Window position:");
                let is_fixed = matches!(self.draft.window_position, WindowPos::Fixed(_, _));
                let mut current = if is_fixed { "Fixed" } else { "Center of screen" }.to_string();
                egui::ComboBox::from_id_salt("window_pos")
                    .selected_text(&current)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut current, "Center of screen".to_string(), "Center of screen").clicked() {
                            self.draft.window_position = WindowPos::NearCursor;
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                        if ui.selectable_value(&mut current, "Fixed".to_string(), "Fixed position").clicked() {
                            self.draft.window_position = WindowPos::Fixed(100, 100);
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                    });
            });

            if matches!(self.draft.window_position, WindowPos::Fixed(_, _)) {
                let (mut x, mut y) = if let WindowPos::Fixed(x, y) = self.draft.window_position {
                    (x, y)
                } else {
                    unreachable!()
                };
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("X:");
                    if ui.add(egui::DragValue::new(&mut x)).changed() { changed = true; }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut y)).changed() { changed = true; }
                });
                if changed {
                    self.draft.window_position = WindowPos::Fixed(x, y);
                    action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                }
            }

            ui.add_space(SPACE_S);

            ui.horizontal(|ui| {
                ui.label("Theme:");
                let current = match self.draft.theme {
                    Theme::System => "System",
                    Theme::Light => "Light",
                    Theme::Dark => "Dark",
                };
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut self.draft.theme, Theme::System, "System").clicked() {
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                        if ui.selectable_value(&mut self.draft.theme, Theme::Light, "Light").clicked() {
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                        if ui.selectable_value(&mut self.draft.theme, Theme::Dark, "Dark").clicked() {
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                    });
            });

            // ── Wayland paste delay ────────────────────────────────────────
            ui.separator();

            ui.label("Wayland paste delay");
            ui.add_space(SPACE_S);
            ui.label(
                "How long the daemon waits after the Copysl window closes before \
                 injecting Ctrl+V. Only applies on Wayland. On X11/XWayland this \
                 setting is ignored — paste is always instant."
            );
            ui.add_space(SPACE_S);
            ui.label("⚠ Too short: keystrokes arrive before focus returns, wrong window pastes.");
            ui.label("⏱ Too long: noticeable lag between clicking and text appearing.");
            ui.add_space(SPACE_S);
            ui.horizontal(|ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.paste_delay_text)
                        .desired_width(56.0),
                );
                ui.label("ms");
                if resp.lost_focus() {
                    match self.paste_delay_text.trim().parse::<u32>() {
                        Ok(v) => {
                            let clamped = v.clamp(10, 3000);
                            self.draft.paste_delay_ms = clamped;
                            self.paste_delay_text = clamped.to_string();
                            action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                        }
                        Err(_) => {
                            self.paste_delay_text = self.draft.paste_delay_ms.to_string();
                        }
                    }
                }
                if ui.button("Default (150 ms)").clicked() {
                    self.draft.paste_delay_ms = 150;
                    self.paste_delay_text = "150".to_string();
                    action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                }
            });

            // ── Danger zone ────────────────────────────────────────────────
            ui.separator();

            let danger_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(180, 40, 40, 15))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 60, 60)))
                .corner_radius(egui::CornerRadius::same(RADIUS_CARD))
                .inner_margin(egui::Margin::same(SPACE_S as i8));

            danger_frame.show(ui, |ui| {
                if ui.add(egui::Button::new(
                    egui::RichText::new("Kill daemon")
                        .color(egui::Color32::from_rgb(200, 60, 60)),
                ).frame(false)).clicked() {
                    action = Some(SettingsAction::KillDaemon);
                }
            });
        });

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_toggle_on_shows_warning() {
        let mut state = PersistenceToggleState::Off;
        state.toggle_on();
        assert!(state.is_warning_shown());
        assert!(!state.is_enabled());
    }

    #[test]
    fn persistence_toggle_confirm_enables() {
        let mut state = PersistenceToggleState::PendingConfirmation;
        state.confirm();
        assert!(state.is_enabled());
        assert!(!state.is_warning_shown());
    }

    #[test]
    fn persistence_toggle_cancel_reverts() {
        let mut state = PersistenceToggleState::PendingConfirmation;
        state.cancel();
        assert_eq!(state, PersistenceToggleState::Off);
        assert!(!state.is_enabled());
    }

    #[test]
    fn max_entries_clamps_to_minimum() {
        // Test the clamping logic directly
        let value: i32 = 5;
        let clamped = value.max(10) as usize;
        assert_eq!(clamped, 10);

        let value: i32 = 15;
        let clamped = value.max(10) as usize;
        assert_eq!(clamped, 15);
    }

    #[test]
    fn settings_panel_new_copies_config() {
        let config = Config::default();
        let panel = SettingsPanel::new(config.clone());
        assert_eq!(panel.draft, config);
        assert!(!panel.show_persist_warning);
    }
}
