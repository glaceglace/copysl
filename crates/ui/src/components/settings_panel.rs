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
}

impl SettingsPanel {
    pub fn new(config: Config) -> Self {
        SettingsPanel {
            draft: config,
            show_persist_warning: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<SettingsAction> {
        let mut action = None;

        ui.heading("Settings");

        // Close button
        if ui.button("×").clicked() {
            action = Some(SettingsAction::Close);
        }

        ui.separator();

        // Max entries
        ui.horizontal(|ui| {
            ui.label("Max history entries:");
            let mut max = self.draft.max_entries as i32;
            if ui.add(egui::DragValue::new(&mut max).range(10..=10000)).changed() {
                self.draft.max_entries = max.max(10) as usize;
                action = Some(SettingsAction::SaveConfig(self.draft.clone()));
            }
        });

        // Persist history
        {
            let mut persist = self.draft.persist_history;
            ui.horizontal(|ui| {
                if ui.checkbox(&mut persist, "Persist history to disk").changed() {
                    if persist && !self.draft.persist_history {
                        // Toggling ON: show warning
                        self.show_persist_warning = true;
                    } else if !persist {
                        self.draft.persist_history = false;
                        action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                    }
                }
            });
            if self.show_persist_warning {
                ui.colored_label(egui::Color32::YELLOW, "Warning: Enabling this saves clipboard contents (including passwords and tokens) to disk. Confirm to proceed.");
                ui.horizontal(|ui| {
                    if ui.button("I understand, enable").clicked() {
                        self.draft.persist_history = true;
                        self.show_persist_warning = false;
                        action = Some(SettingsAction::SaveConfig(self.draft.clone()));
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_persist_warning = false;
                        // draft.persist_history stays false
                    }
                });
            }
        }

        // Autostart
        {
            let mut autostart = self.draft.autostart;
            if ui.checkbox(&mut autostart, "Start on login").changed() {
                self.draft.autostart = autostart;
                action = Some(SettingsAction::SaveConfig(self.draft.clone()));
            }
        }

        // Window position
        ui.horizontal(|ui| {
            ui.label("Window position:");
            let is_fixed = matches!(self.draft.window_position, WindowPos::Fixed(_, _));
            let mut current = if is_fixed { "Fixed" } else { "Near cursor" }.to_string();
            egui::ComboBox::from_id_salt("window_pos")
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut current, "Near cursor".to_string(), "Near cursor").clicked() {
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
                if ui.add(egui::DragValue::new(&mut x)).changed() {
                    changed = true;
                }
                ui.label("Y:");
                if ui.add(egui::DragValue::new(&mut y)).changed() {
                    changed = true;
                }
            });
            if changed {
                self.draft.window_position = WindowPos::Fixed(x, y);
                action = Some(SettingsAction::SaveConfig(self.draft.clone()));
            }
        }

        // Theme
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

        // Paste delay (Wayland only)
        ui.separator();
        ui.label("Wayland paste delay");
        ui.label(
            "How long the daemon waits after the Copieur window closes before \
             injecting Ctrl+V. Only applies on Wayland, where the window manager \
             must return focus to the previous app first. On X11/XWayland this \
             setting is ignored — paste is always instant."
        );
        ui.label("⚠ Too short: keystrokes arrive before focus returns, wrong window pastes.");
        ui.label("⏱ Too long: noticeable lag between clicking and text appearing.");
        ui.horizontal(|ui| {
            let mut delay = self.draft.paste_delay_ms as f32;
            if ui
                .add(
                    egui::Slider::new(&mut delay, 10.0..=3000.0)
                        .suffix(" ms")
                        .step_by(10.0),
                )
                .changed()
            {
                self.draft.paste_delay_ms = delay as u32;
                action = Some(SettingsAction::SaveConfig(self.draft.clone()));
            }
            if ui.button("Default (150 ms)").clicked() {
                self.draft.paste_delay_ms = 150;
                action = Some(SettingsAction::SaveConfig(self.draft.clone()));
            }
        });

        ui.separator();
        if ui.button("Kill daemon").clicked() {
            action = Some(SettingsAction::KillDaemon);
        }

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
