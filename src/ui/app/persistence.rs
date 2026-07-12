use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::Context;

use super::ClevoLedApp;
use crate::dchu::HardwareSnapshot;
use crate::settings::{
    atomic_write_settings, file_modified, load_hardware_snapshot, load_settings, Settings,
};

impl ClevoLedApp {
    pub(super) fn sync_hardware_snapshot(&mut self) {
        if self.last_hardware_sync.elapsed() < Duration::from_millis(600) {
            return;
        }
        self.last_hardware_sync = Instant::now();

        let mtime = file_modified(&self.hardware_snapshot_path);
        if mtime.is_none() || mtime == self.hardware_snapshot_mtime {
            return;
        }

        if let Some(snapshot) = load_hardware_snapshot(&self.hardware_snapshot_path) {
            self.hardware = Some(snapshot);
            self.hardware_status = None;
            self.hardware_snapshot_mtime = mtime;
        }
    }

    pub(in crate::ui) fn mark_settings_dirty(&mut self) {
        self.dirty_settings = true;
    }

    fn apply_external_settings(&mut self, settings: Settings) {
        self.mode = settings.mode;
        self.brightness = settings.brightness;
        self.f0_color = settings.f0_color;
        self.zones = settings.zones;
        self.fan_curves = settings.fan_curves.clone();
        self.fan_curve_draft = settings.fan_curves;
        self.fan_curve_selection = None;
        self.battery_strategy = settings.battery_strategy;
        self.language_preference = settings.language;
        self.language = settings.language.resolved();
        self.theme_color = settings.theme_color;
    }

    pub(super) fn sync_external_settings(&mut self) {
        if self.dirty_settings {
            return;
        }

        let mtime = file_modified(&self.settings_path);
        if mtime.is_none() || mtime == self.settings_mtime {
            return;
        }

        let settings = load_settings(&self.settings_path);
        self.apply_external_settings(settings);
        self.settings_mtime = mtime;
    }

    pub(super) fn update_window_position(&mut self, ctx: &Context) {
        let position = ctx.input(|input| {
            input
                .viewport()
                .outer_rect
                .map(|rect| [rect.min.x, rect.min.y])
        });

        if let Some(position) = position {
            let changed = self
                .window_pos
                .map(|old| (old[0] - position[0]).abs() > 0.5 || (old[1] - position[1]).abs() > 0.5)
                .unwrap_or(true);
            if changed {
                self.window_pos = Some(position);
                self.dirty_window_position = true;
            }
        }
    }

    pub(in crate::ui) fn persist_settings_if_due(&mut self, force: bool) {
        if !self.dirty_settings && !self.dirty_window_position && !force {
            return;
        }
        if !force && self.last_settings_save.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.persist_settings();
    }

    fn persist_settings(&mut self) {
        let mut settings = if self.dirty_settings {
            Settings {
                mode: self.mode,
                brightness: self.brightness,
                f0_color: self.f0_color,
                zones: self.selected_zones(),
                fan_curves: self.fan_curves.clone(),
                battery_strategy: self.battery_strategy.clone(),
                language: self.language_preference,
                theme_color: self.theme_color,
                window_pos: self.window_pos,
            }
        } else {
            load_settings(&self.settings_path)
        };

        settings.window_pos = self.window_pos;
        settings = settings.sanitized();
        let should_apply_local_state = self.dirty_settings;

        match atomic_write_settings(&self.settings_path, &settings) {
            Ok(()) => {
                self.dirty_settings = false;
                self.dirty_window_position = false;
                self.last_settings_save = Instant::now();
                self.settings_mtime = file_modified(&self.settings_path);
                if !should_apply_local_state {
                    self.apply_external_settings(settings);
                }
            }
            Err(err) => {
                eprintln!("Failed to write {}: {err}", self.settings_path.display());
                self.last_settings_save = Instant::now();
            }
        }
    }
}

pub(super) fn wait_for_hardware_snapshot(
    path: &Path,
    timeout: Duration,
) -> Option<HardwareSnapshot> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(snapshot) = load_hardware_snapshot(path) {
            return Some(snapshot);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(75));
    }
}
