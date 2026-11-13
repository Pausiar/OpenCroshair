//! App shell: owns `AppState` and the background workers (overlay, hotkeys, tray, detection,
//! save worker). Handles events in `logic`, draws the sidebar, the active page and toasts in `ui`.
use crate::hotkeys::{HotkeyAction, HotkeyManager};
use crate::overlay::OverlayHandle;
use crate::profiles::detection::{DetectionEvent, Detector};
use crate::profiles::manager::{Resolution, resolve_preset_id};
use crate::state::{AppState, Page};
use crate::storage::config::{LoadedData, Paths, SaveJob, SaveWorker, ThemeMode};
use crate::system::startup;
use crate::system::tray::{Tray, TrayEvent};
use crate::ui::titlebar::{self, TitleBarAction};
use crate::ui::{dashboard, editor, navigation, presets, profiles, settings, theme, widgets};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Margin, Order, Pos2, Rect, RichText, Shadow, Stroke, ViewportCommand,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Idle delay after the last change before settings are written.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
/// Upper bound for how long a steady stream of changes may postpone a save.
const SAVE_MAX_WAIT: Duration = Duration::from_millis(2000);
/// Wake-up period for polling hotkeys and tray events (also used while the window is hidden).
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const TOAST_TTL: Duration = Duration::from_secs(4);
const TOAST_TTL_ERROR: Duration = Duration::from_secs(7);

/// Indices of `presets` with favorites first, keeping the original order within each group.
pub fn favorites_first_order(presets: &[crate::crosshair::model::Preset]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..presets.len()).collect();
    idx.sort_by_key(|&i| !presets[i].favorite);
    idx
}

/// Next index when stepping by `delta` through `len` items, wrapping around.
/// With no current selection, forward starts at the first item and backward at the last.
/// `None` when there is nothing to step through.
pub fn cycle_index(len: usize, current: Option<usize>, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let len_i = len as isize;
    Some(match current {
        Some(i) => (i as isize + delta).rem_euclid(len_i) as usize,
        None if delta >= 0 => 0,
        None => len - 1,
    })
}

/// Stable key for a hotkey action, shared with `settings::HOTKEY_ROWS` and `AppState::hotkey_errors`.
pub fn action_key(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::ToggleCrosshair => "toggle_crosshair",
        HotkeyAction::ToggleOverlay => "toggle_overlay",
        HotkeyAction::NextPreset => "next_preset",
        HotkeyAction::PrevPreset => "prev_preset",
        HotkeyAction::ToggleAutoProfiles => "toggle_auto_profiles",
    }
}

pub struct App {
    state: AppState,
    saver: SaveWorker,
    hotkeys: Option<HotkeyManager>,
    tray: Option<Tray>,
    detector: Detector,
    /// Auto-profile state the detector was last switched to.
    auto_applied: bool,
    interval_applied: u32,
    /// Id of the profile currently applied (`state.active_profile` holds its name).
    active_profile_id: Option<String>,
    /// Last (crosshair_visible, overlay_enabled) pushed to the tray.
    tray_states: Option<(bool, bool)>,
    /// Theme mode last installed on the egui context (compared with `settings.theme` each frame).
    applied_theme: ThemeMode,
    quitting: bool,
    pending_toasts: VecDeque<(String, bool)>,
    /// Toast currently shown, with the time it first appeared.
    toast_seen: Option<(String, bool, Instant)>,
    save_pending: bool,
    dirty_since: Option<Instant>,
    last_change: Option<Instant>,
}

impl App {
    /// Build the app: theme, overlay thread, hotkeys, detector, tray and the save worker.
    /// `start_hidden` is only honored when the tray exists (otherwise the window stays visible).
    pub fn new(cc: &eframe::CreationContext<'_>, paths: Paths, data: LoadedData, start_hidden: bool) -> Self {
        let ctx = cc.egui_ctx.clone();
        crate::ui::fonts::install(&ctx);
        theme::apply(&ctx, data.settings.theme);

        let mut pending_toasts: VecDeque<(String, bool)> = data.warnings.iter().map(|w| (w.clone(), true)).collect();

        let repaint_ctx = ctx.clone();
        let detector = Detector::spawn(Box::new(move || repaint_ctx.request_repaint()));
        let settings = data.settings.clone();
        detector.set_interval_ms(settings.detection_interval_ms);
        detector.set_enabled(settings.auto_profiles);

        let tray = match Tray::new() {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!("tray unavailable: {e}");
                pending_toasts.push_back((format!("System tray unavailable: {e}"), true));
                None
            }
        };
        if start_hidden && tray.is_none() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        }

        let mut hotkeys = match HotkeyManager::new() {
            Ok(hk) => Some(hk),
            Err(e) => {
                pending_toasts.push_back((format!("Global hotkeys unavailable: {e}"), true));
                None
            }
        };
        let hotkey_errors = register_hotkeys(&mut hotkeys, &settings);

        let state = AppState {
            editing: settings.active_crosshair.clone(),
            settings: settings.clone(),
            presets: data.presets,
            profiles: data.profiles,
            page: Page::Overview,
            active_profile: None,
            status: None,
            dirty: false,
            overlay: OverlayHandle::spawn(),
            hotkey_errors,
            hotkeys_changed: false,
            startup_changed: false,
        };
        state.push_overlay();

        let saver = SaveWorker::spawn(paths);
        ctx.request_repaint();

        Self {
            state,
            saver,
            hotkeys,
            tray,
            detector,
            auto_applied: settings.auto_profiles,
            interval_applied: settings.detection_interval_ms,
            active_profile_id: None,
            tray_states: None,
            applied_theme: settings.theme,
            quitting: false,
            pending_toasts,
            toast_seen: None,
            save_pending: false,
            dirty_since: None,
            last_change: None,
        }
    }

    fn close_to_tray(&self) -> bool {
        self.state.settings.minimize_to_tray && self.tray.is_some()
    }

    fn handle_action(&mut self, action: HotkeyAction) {
        match action {
            HotkeyAction::ToggleCrosshair => self.toggle_crosshair(),
            HotkeyAction::ToggleOverlay => self.toggle_overlay(),
            HotkeyAction::NextPreset => self.cycle_preset(1),
            HotkeyAction::PrevPreset => self.cycle_preset(-1),
            HotkeyAction::ToggleAutoProfiles => {
                self.state.settings.auto_profiles = !self.state.settings.auto_profiles;
                self.state.dirty = true;
            }
        }
    }

    fn toggle_crosshair(&mut self) {
        self.state.settings.crosshair_visible = !self.state.settings.crosshair_visible;
        self.state.push_overlay();
        self.state.dirty = true;
        let msg = if self.state.settings.crosshair_visible {
            "Crosshair shown"
        } else {
            "Crosshair hidden"
        };
        self.state.toast(msg, false);
    }

    fn toggle_overlay(&mut self) {
        self.state.settings.overlay_enabled = !self.state.settings.overlay_enabled;
        self.state.push_overlay();
        self.state.dirty = true;
        let msg = if self.state.settings.overlay_enabled {
            "Overlay on"
        } else {
            "Overlay off"
        };
        self.state.toast(msg, false);
    }

    /// Step through presets in favorites-first order (stable within each group).
    fn cycle_preset(&mut self, delta: isize) {
        let order = favorites_first_order(&self.state.presets);
        let current = self
            .state
            .settings
            .active_preset_id
            .as_ref()
            .and_then(|active| order.iter().position(|&i| &self.state.presets[i].id == active));
        let Some(next) = cycle_index(order.len(), current, delta) else {
            self.state.toast("No presets to cycle through", true);
            return;
        };
        let preset = &self.state.presets[order[next]];
        let (id, name) = (preset.id.clone(), preset.name.clone());
        self.state.apply_preset(&id);
        self.state.toast(format!("Preset: {name}"), false);
    }

    /// React to a foreground executable change (or None when no profiled app is in front).
    fn apply_foreground(&mut self, exe: Option<&str>) {
        let resolution = resolve_preset_id(
            &self.state.profiles,
            exe,
            self.state.settings.default_profile_preset_id.as_deref(),
            self.active_profile_id.as_deref(),
        );
        match resolution {
            Resolution::Profile { profile_id, preset_id } => {
                let name = self
                    .state
                    .profiles
                    .iter()
                    .find(|p| p.id == profile_id)
                    .map(|p| p.name.clone());
                self.active_profile_id = Some(profile_id);
                self.state.active_profile = name;
                if let Some(pid) = preset_id {
                    if self.state.presets.iter().any(|p| p.id == pid) {
                        self.state.apply_preset(&pid);
                    } else {
                        self.state
                            .toast("The preset for this game profile no longer exists", true);
                    }
                }
            }
            Resolution::Revert { preset_id } => {
                self.active_profile_id = None;
                self.state.active_profile = None;
                if let Some(pid) = preset_id.filter(|pid| self.state.presets.iter().any(|p| &p.id == pid)) {
                    self.state.apply_preset(&pid);
                }
            }
            Resolution::Keep => {}
        }
    }

    /// Apply auto-profile on/off and detection interval changes from settings.
    fn sync_detection(&mut self) {
        let want = self.state.settings.auto_profiles;
        if want != self.auto_applied {
            self.auto_applied = want;
            self.detector.set_enabled(want);
            if !want && self.active_profile_id.is_some() {
                self.apply_foreground(None);
            }
            if !want {
                self.state.active_profile = None;
                self.active_profile_id = None;
            }
        }
        let ms = self.state.settings.detection_interval_ms;
        if ms != self.interval_applied {
            self.interval_applied = ms;
            self.detector.set_interval_ms(ms);
        }
    }

    fn poll_detection(&mut self) {
        while let Some(event) = self.detector.try_recv() {
            let DetectionEvent::Changed(exe) = event;
            if self.state.settings.auto_profiles {
                self.apply_foreground(exe.as_deref());
            }
        }
    }

    fn poll_hotkeys(&mut self) {
        let actions = match self.hotkeys.as_mut() {
            Some(hk) => hk.poll(),
            None => Vec::new(),
        };
        for action in actions {
            self.handle_action(action);
        }
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        let events = match self.tray.as_mut() {
            Some(t) => t.poll(),
            None => return,
        };
        for event in events {
            match event {
                TrayEvent::Show => {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                }
                TrayEvent::ToggleCrosshair => self.toggle_crosshair(),
                TrayEvent::ToggleOverlay => self.toggle_overlay(),
                TrayEvent::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }

    fn sync_tray(&mut self) {
        let states = (
            self.state.settings.crosshair_visible,
            self.state.settings.overlay_enabled,
        );
        if self.tray_states != Some(states) {
            if let Some(t) = self.tray.as_mut() {
                t.set_states(states.0, states.1);
            }
            self.tray_states = Some(states);
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if !self.quitting && self.close_to_tray() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        } else {
            self.quitting = true;
        }
    }

    /// Install the theme on the context when `settings.theme` differs from the applied mode.
    /// Covers the title bar toggle and the Settings page alike; a change marks settings dirty.
    fn sync_theme(&mut self, ctx: &egui::Context) {
        if self.applied_theme != self.state.settings.theme {
            self.applied_theme = self.state.settings.theme;
            theme::apply(ctx, self.applied_theme);
            self.state.dirty = true;
        }
    }

    /// Apply a title bar request. Close goes through the window's close request, so close-to-tray
    /// and the quit path behave exactly as for the system close button.
    fn apply_titlebar_action(&mut self, ctx: &egui::Context, action: TitleBarAction) {
        match action {
            TitleBarAction::None | TitleBarAction::ToggleTheme => {}
            TitleBarAction::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            TitleBarAction::ToggleMaximize => {
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            TitleBarAction::Close => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    fn apply_pending_hotkeys(&mut self) {
        if self.state.hotkeys_changed {
            self.state.hotkeys_changed = false;
            self.state.hotkey_errors = register_hotkeys(&mut self.hotkeys, &self.state.settings);
        }
    }

    fn apply_pending_startup(&mut self) {
        if !self.state.startup_changed {
            return;
        }
        self.state.startup_changed = false;
        let want = self.state.settings.launch_at_startup;
        match startup::set_enabled(want) {
            Ok(()) => {
                let msg = if want {
                    "Launch at Windows startup is on"
                } else {
                    "Launch at Windows startup is off"
                };
                self.state.toast(msg, false);
            }
            Err(e) => {
                // Show the real state, not the requested one.
                self.state.settings.launch_at_startup = startup::is_enabled();
                self.state.dirty = true;
                self.state.toast(format!("Could not change startup setting: {e}"), true);
            }
        }
    }

    /// Debounced save: a snapshot is queued to the worker once changes have been quiet for
    /// `SAVE_DEBOUNCE` (or `SAVE_MAX_WAIT` since the first change). No disk I/O on this thread.
    fn autosave(&mut self, now: Instant) {
        if self.state.dirty {
            self.state.dirty = false;
            self.dirty_since.get_or_insert(now);
            self.last_change = Some(now);
            self.save_pending = true;
        }
        if !self.save_pending {
            return;
        }
        let quiet = self.last_change.map(|t| now.duration_since(t)).unwrap_or_default();
        let waiting = self.dirty_since.map(|t| now.duration_since(t)).unwrap_or_default();
        if quiet >= SAVE_DEBOUNCE || waiting >= SAVE_MAX_WAIT {
            self.queue_save();
        }
    }

    fn queue_save(&mut self) {
        self.saver.save(SaveJob {
            settings: Some(self.state.settings.clone()),
            presets: Some(self.state.presets.clone()),
            profiles: Some(self.state.profiles.clone()),
        });
        self.save_pending = false;
        self.dirty_since = None;
        self.last_change = None;
    }

    fn update_toasts(&mut self, now: Instant) {
        for err in self.saver.take_errors() {
            self.pending_toasts.push_back((err, true));
        }
        if self.state.status.is_none()
            && let Some(next) = self.pending_toasts.pop_front()
        {
            self.state.status = Some(next);
        }
        match (&self.state.status, &self.toast_seen) {
            (None, _) => self.toast_seen = None,
            (Some((msg, err)), Some((seen, seen_err, _))) if msg == seen && err == seen_err => {}
            (Some((msg, err)), _) => self.toast_seen = Some((msg.clone(), *err, now)),
        }
        if let Some((_, err, since)) = &self.toast_seen {
            let ttl = if *err { TOAST_TTL_ERROR } else { TOAST_TTL };
            if now.duration_since(*since) >= ttl {
                self.state.status = None;
                self.toast_seen = None;
            }
        }
    }

    fn toast_remaining(&self, now: Instant) -> Option<Duration> {
        let (_, err, since) = self.toast_seen.as_ref()?;
        let ttl = if *err { TOAST_TTL_ERROR } else { TOAST_TTL };
        Some(ttl.saturating_sub(now.duration_since(*since)))
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let Some((msg, is_error)) = self.state.status.clone() else {
            return;
        };
        let p = theme::palette(ctx);
        let accent = if is_error { p.danger } else { p.accent };
        let mut dismiss = false;
        egui::Area::new(widgets::state_id("oc_toast"))
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-24.0, -20.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                let shadow_alpha = if ui.visuals().dark_mode { 110 } else { 28 };
                let out = Frame::new()
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.border))
                    .corner_radius(theme::RADIUS)
                    .inner_margin(Margin {
                        left: 18,
                        right: 12,
                        top: 10,
                        bottom: 10,
                    })
                    .shadow(Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(shadow_alpha),
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(440.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&msg).color(p.text));
                            if widgets::ghost_button(ui, "Dismiss").clicked() {
                                dismiss = true;
                            }
                        });
                    });
                // Accent stripe along the left edge, following the card's rounded corners.
                let r = out.response.rect;
                let stripe = Rect::from_min_max(r.left_top(), Pos2::new(r.left() + 4.0, r.bottom()));
                let corners = CornerRadius {
                    nw: 8,
                    sw: 8,
                    ne: 0,
                    se: 0,
                };
                ui.painter().rect_filled(stripe, corners, accent);
            });
        if dismiss {
            self.state.status = None;
        }
    }
}

/// Register all hotkeys and return the per-action errors as (action key, message).
fn register_hotkeys(
    hotkeys: &mut Option<HotkeyManager>,
    settings: &crate::storage::config::Settings,
) -> Vec<(String, String)> {
    let Some(hk) = hotkeys.as_mut() else {
        return vec![(
            "all".to_string(),
            "Global hotkeys are unavailable on this system.".to_string(),
        )];
    };
    hk.register_all(&settings.hotkeys)
        .into_iter()
        .filter_map(|(action, result)| result.err().map(|e| (action_key(action).to_string(), e)))
        .collect()
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        self.handle_close_request(ctx);
        self.poll_tray(ctx);
        self.poll_hotkeys();
        self.poll_detection();
        self.sync_detection();
        self.apply_pending_hotkeys();
        self.apply_pending_startup();
        self.sync_tray();
        self.autosave(now);
        self.update_toasts(now);

        // Low-frequency wake-up keeps hotkeys and tray responsive, including while hidden.
        let mut next = POLL_INTERVAL;
        if let Some(rem) = self.toast_remaining(now) {
            next = next.min(rem.max(Duration::from_millis(50)));
        }
        if self.save_pending {
            next = next.min(SAVE_DEBOUNCE);
        }
        ctx.request_repaint_after(next);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.sync_theme(&ctx);
        let custom_bar = !self.state.settings.use_system_title_bar;

        if custom_bar {
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            let action = titlebar::show_ui(ui, &mut self.state.settings.theme, maximized);
            self.sync_theme(&ctx);
            self.apply_titlebar_action(&ctx, action);
        }

        if self.state.settings.onboarding_done {
            egui::Panel::left("oc_sidebar")
                .exact_size(220.0)
                .resizable(false)
                .frame(
                    Frame::new()
                        .fill(theme::palette(&ctx).surface)
                        .inner_margin(Margin::symmetric(14, 16)),
                )
                .show(ui, |ui| navigation::show(ui, &mut self.state));

            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(theme::palette(&ctx).bg)
                        .inner_margin(Margin::symmetric(28, 22)),
                )
                .show(ui, |ui| match self.state.page {
                    Page::Overview => dashboard::show(ui, &mut self.state),
                    Page::Editor => editor::show(ui, &mut self.state),
                    Page::Presets => presets::show(ui, &mut self.state),
                    Page::Profiles => profiles::show(ui, &mut self.state),
                    Page::Settings => settings::show(ui, &mut self.state),
                });
        } else {
            egui::CentralPanel::default()
                .frame(Frame::new().fill(theme::palette(&ctx).bg))
                .show(ui, |ui| crate::ui::onboarding::show(ui, &mut self.state));
        }

        if custom_bar {
            titlebar::resize_handles(&ctx);
        }
        self.draw_toast(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.state.dirty || self.save_pending {
            self.state.dirty = false;
            self.queue_save();
        }
        self.saver.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosshair::model::Preset;

    fn preset(id: &str, favorite: bool) -> Preset {
        Preset {
            id: id.into(),
            favorite,
            ..Preset::default()
        }
    }

    #[test]
    fn cycle_wraps_both_directions() {
        assert_eq!(cycle_index(3, Some(2), 1), Some(0));
        assert_eq!(cycle_index(3, Some(0), -1), Some(2));
        assert_eq!(cycle_index(3, Some(1), 1), Some(2));
    }

    #[test]
    fn cycle_without_selection_and_empty() {
        assert_eq!(cycle_index(4, None, 1), Some(0));
        assert_eq!(cycle_index(4, None, -1), Some(3));
        assert_eq!(cycle_index(0, None, 1), None);
        assert_eq!(cycle_index(0, Some(0), -1), None);
    }

    #[test]
    fn favorites_come_first_and_keep_order() {
        let list = vec![
            preset("a", false),
            preset("b", true),
            preset("c", false),
            preset("d", true),
        ];
        assert_eq!(favorites_first_order(&list), vec![1, 3, 0, 2]);
        assert_eq!(favorites_first_order(&[]), Vec::<usize>::new());
    }

    #[test]
    fn action_keys_match_settings_rows() {
        use crate::ui::settings::HOTKEY_ROWS;
        let actions = [
            HotkeyAction::ToggleCrosshair,
            HotkeyAction::ToggleOverlay,
            HotkeyAction::NextPreset,
            HotkeyAction::PrevPreset,
            HotkeyAction::ToggleAutoProfiles,
        ];
        for (action, row) in actions.iter().zip(HOTKEY_ROWS.iter()) {
            assert_eq!(action_key(*action), row.0);
        }
    }
}
