//! Global hotkeys (agent).
//!
//! Parsing, formatting and conflict detection are pure and compile everywhere (see
//! [`HotkeySpec`] and [`plan_registrations`]). OS registration goes through the
//! `global-hotkey` crate on Windows only; other platforms get a stub that reports
//! "only available on Windows" for every valid binding.

use crate::storage::config::HotkeySettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleCrosshair,
    ToggleOverlay,
    NextPreset,
    PrevPreset,
    ToggleAutoProfiles,
}

impl HotkeyAction {
    /// Every action, in the order they are registered and shown in Settings.
    pub const ALL: [HotkeyAction; 5] = [
        HotkeyAction::ToggleCrosshair,
        HotkeyAction::ToggleOverlay,
        HotkeyAction::NextPreset,
        HotkeyAction::PrevPreset,
        HotkeyAction::ToggleAutoProfiles,
    ];

    /// English label for UI and error messages.
    pub fn label(self) -> &'static str {
        match self {
            HotkeyAction::ToggleCrosshair => "Toggle crosshair",
            HotkeyAction::ToggleOverlay => "Toggle overlay",
            HotkeyAction::NextPreset => "Next preset",
            HotkeyAction::PrevPreset => "Previous preset",
            HotkeyAction::ToggleAutoProfiles => "Toggle auto profiles",
        }
    }

    /// The configured key combination string for this action.
    pub fn binding(self, settings: &HotkeySettings) -> &str {
        match self {
            HotkeyAction::ToggleCrosshair => &settings.toggle_crosshair,
            HotkeyAction::ToggleOverlay => &settings.toggle_overlay,
            HotkeyAction::NextPreset => &settings.next_preset,
            HotkeyAction::PrevPreset => &settings.prev_preset,
            HotkeyAction::ToggleAutoProfiles => &settings.toggle_auto_profiles,
        }
    }
}

/// Errors from parsing a key combination string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyError {
    #[error("no key combination is set")]
    Empty,
    #[error("there is an empty part between two '+' signs")]
    EmptyPart,
    #[error("\"{0}\" is not a recognized key")]
    UnknownKey(String),
    #[error("add a main key after the modifiers, such as F1 or R")]
    MissingKey,
    #[error("only one main key is allowed (found \"{0}\" and \"{1}\")")]
    MultipleKeys(String, String),
    #[error("modifiers must come before the main key (\"{0}\" is after it)")]
    ModifierAfterKey(String),
    #[error("\"{0}\" is listed more than once")]
    RepeatedModifier(String),
    #[error("add Ctrl, Alt, Shift or Win; only F1 to F24 can be used on their own")]
    NeedsModifier,
}

/// Platform-independent description of a key combination. `key` is the canonical
/// name used for display, e.g. "F1", "A", "5", "Right", "Esc", "[".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeySpec {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: String,
}

impl HotkeySpec {
    /// Canonical text, e.g. "Ctrl+Alt+F1". Modifier order is Ctrl, Alt, Shift, Win.
    pub fn format(&self) -> String {
        let mut parts: Vec<&str> = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Win");
        }
        parts.push(&self.key);
        parts.join("+")
    }
}

/// Canonical key names with lowercase aliases accepted on input.
const NAMED_KEYS: &[(&str, &[&str])] = &[
    ("Space", &["spacebar"]),
    ("Enter", &["return"]),
    ("Esc", &["escape"]),
    ("Tab", &[]),
    ("Backspace", &["back"]),
    ("Insert", &["ins"]),
    ("Delete", &["del"]),
    ("Home", &[]),
    ("End", &[]),
    ("PageUp", &["pgup"]),
    ("PageDown", &["pgdn"]),
    ("Up", &["arrowup"]),
    ("Down", &["arrowdown"]),
    ("Left", &["arrowleft"]),
    ("Right", &["arrowright"]),
    ("PrintScreen", &["prtsc", "printscr"]),
    ("Pause", &["break"]),
    ("ScrollLock", &[]),
    ("NumLock", &[]),
    ("Numpad0", &["num0"]),
    ("Numpad1", &["num1"]),
    ("Numpad2", &["num2"]),
    ("Numpad3", &["num3"]),
    ("Numpad4", &["num4"]),
    ("Numpad5", &["num5"]),
    ("Numpad6", &["num6"]),
    ("Numpad7", &["num7"]),
    ("Numpad8", &["num8"]),
    ("Numpad9", &["num9"]),
    ("-", &["minus"]),
    ("=", &["equals", "equal"]),
    ("[", &["openbracket", "bracketleft"]),
    ("]", &["closebracket", "bracketright"]),
    ("\\", &["backslash"]),
    (";", &["semicolon"]),
    ("'", &["quote", "apostrophe"]),
    ("`", &["backtick", "backquote"]),
    (",", &["comma"]),
    (".", &["period"]),
    ("/", &["slash"]),
];

/// Canonical spelling of a single key token, or None if it is not a known key.
fn canonical_key(token: &str) -> Option<String> {
    let lower = token.to_ascii_lowercase();
    let mut chars = lower.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase().to_string());
        }
        if c.is_ascii_digit() {
            return Some(c.to_string());
        }
    }
    if let Some(n) = lower
        .strip_prefix('f')
        .filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|d| d.parse::<u8>().ok())
        .filter(|n| (1..=24).contains(n))
    {
        return Some(format!("F{n}"));
    }
    NAMED_KEYS
        .iter()
        .find(|(name, aliases)| name.eq_ignore_ascii_case(token) || aliases.contains(&lower.as_str()))
        .map(|(name, _)| (*name).to_string())
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('F').is_some_and(|n| n.parse::<u8>().is_ok())
}

/// Parse text such as "Ctrl+Alt+F1" or "shift + right" into a [`HotkeySpec`].
///
/// Rules: modifiers (Ctrl/Control, Alt, Shift, Win/Windows/Meta) come first, exactly one main
/// key follows, and the combination needs at least one modifier unless the key is F1 to F24.
pub fn parse_spec(input: &str) -> Result<HotkeySpec, HotkeyError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(HotkeyError::Empty);
    }
    let mut spec = HotkeySpec {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
        key: String::new(),
    };
    let mut seen_mods: Vec<&'static str> = Vec::new();
    let tokens: Vec<&str> = text.split('+').map(str::trim).collect();
    let last = tokens.len() - 1;

    for (i, token) in tokens.iter().copied().enumerate() {
        if token.is_empty() {
            return if i == last {
                Err(HotkeyError::MissingKey)
            } else {
                Err(HotkeyError::EmptyPart)
            };
        }
        let modifier = match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some("Ctrl"),
            "alt" => Some("Alt"),
            "shift" => Some("Shift"),
            "win" | "windows" | "meta" | "super" => Some("Win"),
            _ => None,
        };
        if let Some(name) = modifier {
            if !spec.key.is_empty() {
                return Err(HotkeyError::ModifierAfterKey(token.to_string()));
            }
            if seen_mods.contains(&name) {
                return Err(HotkeyError::RepeatedModifier(token.to_string()));
            }
            seen_mods.push(name);
            match name {
                "Ctrl" => spec.ctrl = true,
                "Alt" => spec.alt = true,
                "Shift" => spec.shift = true,
                _ => spec.win = true,
            }
            continue;
        }
        if !spec.key.is_empty() {
            return Err(HotkeyError::MultipleKeys(spec.key.clone(), token.to_string()));
        }
        spec.key = canonical_key(token).ok_or_else(|| HotkeyError::UnknownKey(token.to_string()))?;
    }

    if spec.key.is_empty() {
        return Err(HotkeyError::MissingKey);
    }
    if seen_mods.is_empty() && !is_function_key(&spec.key) {
        return Err(HotkeyError::NeedsModifier);
    }
    Ok(spec)
}

/// Parse and re-format text into its canonical spelling, e.g. "ctrl+alt+f1" -> "Ctrl+Alt+F1".
pub fn normalize_hotkey(input: &str) -> Result<String, HotkeyError> {
    parse_spec(input).map(|s| s.format())
}

/// Decide what to register for every action. Each entry is either the parsed combination or a
/// clear message: a parse error, an empty binding, or a clash with an earlier action in the
/// list ("already used by ..."). Actions are checked in [`HotkeyAction::ALL`] order, so the
/// first action keeps a contested combination.
pub fn plan_registrations(settings: &HotkeySettings) -> Vec<(HotkeyAction, Result<HotkeySpec, String>)> {
    let mut claimed: Vec<(HotkeySpec, HotkeyAction)> = Vec::new();
    let mut out = Vec::with_capacity(HotkeyAction::ALL.len());
    for action in HotkeyAction::ALL {
        let text = action.binding(settings).trim();
        if text.is_empty() {
            out.push((action, Err(HotkeyError::Empty.to_string())));
            continue;
        }
        let spec = match parse_spec(text) {
            Ok(spec) => spec,
            Err(e) => {
                out.push((action, Err(format!("Invalid key combination \"{text}\": {e}."))));
                continue;
            }
        };
        if let Some((_, first)) = claimed.iter().find(|(s, _)| *s == spec) {
            out.push((
                action,
                Err(format!("Already used by \"{}\" in this app.", first.label())),
            ));
            continue;
        }
        claimed.push((spec.clone(), action));
        out.push((action, Ok(spec)));
    }
    out
}

/// Turn an egui key press into a combination string for the Settings UI.
///
/// Reads the first non-repeated key-down event in `input` and returns its canonical text, e.g.
/// "Ctrl+Shift+Right". Returns None when no key was pressed, the key has no hotkey name, or the
/// combination would be rejected by [`parse_spec`] (for example a bare letter).
pub fn capture_hotkey_string(input: &eframe::egui::InputState) -> Option<String> {
    input.events.iter().find_map(|event| match event {
        eframe::egui::Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } => {
            let spec = HotkeySpec {
                ctrl: modifiers.ctrl,
                alt: modifiers.alt,
                shift: modifiers.shift,
                win: false,
                key: canonical_key(key.name())?,
            };
            parse_spec(&spec.format()).ok().map(|_| spec.format())
        }
        _ => None,
    })
}

#[cfg(windows)]
mod platform {
    use super::{HotkeyAction, HotkeySpec, plan_registrations};
    use crate::storage::config::HotkeySettings;
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    const LETTERS: [Code; 26] = [
        Code::KeyA,
        Code::KeyB,
        Code::KeyC,
        Code::KeyD,
        Code::KeyE,
        Code::KeyF,
        Code::KeyG,
        Code::KeyH,
        Code::KeyI,
        Code::KeyJ,
        Code::KeyK,
        Code::KeyL,
        Code::KeyM,
        Code::KeyN,
        Code::KeyO,
        Code::KeyP,
        Code::KeyQ,
        Code::KeyR,
        Code::KeyS,
        Code::KeyT,
        Code::KeyU,
        Code::KeyV,
        Code::KeyW,
        Code::KeyX,
        Code::KeyY,
        Code::KeyZ,
    ];
    const DIGITS: [Code; 10] = [
        Code::Digit0,
        Code::Digit1,
        Code::Digit2,
        Code::Digit3,
        Code::Digit4,
        Code::Digit5,
        Code::Digit6,
        Code::Digit7,
        Code::Digit8,
        Code::Digit9,
    ];
    const NUMPAD: [Code; 10] = [
        Code::Numpad0,
        Code::Numpad1,
        Code::Numpad2,
        Code::Numpad3,
        Code::Numpad4,
        Code::Numpad5,
        Code::Numpad6,
        Code::Numpad7,
        Code::Numpad8,
        Code::Numpad9,
    ];
    const FKEYS: [Code; 24] = [
        Code::F1,
        Code::F2,
        Code::F3,
        Code::F4,
        Code::F5,
        Code::F6,
        Code::F7,
        Code::F8,
        Code::F9,
        Code::F10,
        Code::F11,
        Code::F12,
        Code::F13,
        Code::F14,
        Code::F15,
        Code::F16,
        Code::F17,
        Code::F18,
        Code::F19,
        Code::F20,
        Code::F21,
        Code::F22,
        Code::F23,
        Code::F24,
    ];

    /// Map a canonical key name (as produced by `parse_spec`) to a `Code`.
    fn code_for(key: &str) -> Option<Code> {
        let mut chars = key.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if c.is_ascii_uppercase() {
                return Some(LETTERS[(c as u8 - b'A') as usize]);
            }
            if c.is_ascii_digit() {
                return Some(DIGITS[(c as u8 - b'0') as usize]);
            }
        }
        if let Some(n) = key.strip_prefix('F').and_then(|n| n.parse::<usize>().ok()) {
            return FKEYS.get(n.checked_sub(1)?).copied();
        }
        if let Some(n) = key.strip_prefix("Numpad").and_then(|n| n.parse::<usize>().ok()) {
            return NUMPAD.get(n).copied();
        }
        Some(match key {
            "Space" => Code::Space,
            "Enter" => Code::Enter,
            "Esc" => Code::Escape,
            "Tab" => Code::Tab,
            "Backspace" => Code::Backspace,
            "Insert" => Code::Insert,
            "Delete" => Code::Delete,
            "Home" => Code::Home,
            "End" => Code::End,
            "PageUp" => Code::PageUp,
            "PageDown" => Code::PageDown,
            "Up" => Code::ArrowUp,
            "Down" => Code::ArrowDown,
            "Left" => Code::ArrowLeft,
            "Right" => Code::ArrowRight,
            "PrintScreen" => Code::PrintScreen,
            "Pause" => Code::Pause,
            "ScrollLock" => Code::ScrollLock,
            "NumLock" => Code::NumLock,
            "-" => Code::Minus,
            "=" => Code::Equal,
            "[" => Code::BracketLeft,
            "]" => Code::BracketRight,
            "\\" => Code::Backslash,
            ";" => Code::Semicolon,
            "'" => Code::Quote,
            "`" => Code::Backquote,
            "," => Code::Comma,
            "." => Code::Period,
            "/" => Code::Slash,
            _ => return None,
        })
    }

    fn to_hotkey(spec: &HotkeySpec) -> Option<HotKey> {
        let mut mods = Modifiers::empty();
        if spec.ctrl {
            mods |= Modifiers::CONTROL;
        }
        if spec.alt {
            mods |= Modifiers::ALT;
        }
        if spec.shift {
            mods |= Modifiers::SHIFT;
        }
        if spec.win {
            mods |= Modifiers::META;
        }
        Some(HotKey::new(Some(mods), code_for(&spec.key)?))
    }

    /// Parse text such as "Ctrl+Alt+F1" into a registrable `HotKey`.
    pub fn parse_hotkey(input: &str) -> Result<HotKey, super::HotkeyError> {
        let spec = super::parse_spec(input)?;
        to_hotkey(&spec).ok_or_else(|| super::HotkeyError::UnknownKey(spec.key.clone()))
    }

    pub struct HotkeyManager {
        inner: GlobalHotKeyManager,
        registered: Vec<(HotkeyAction, HotKey)>,
    }

    impl HotkeyManager {
        /// Must be created on the thread that pumps Win32 messages (eframe's main thread).
        pub fn new() -> Result<Self, String> {
            let inner = GlobalHotKeyManager::new().map_err(|e| format!("Global hotkeys are unavailable: {e}"))?;
            Ok(Self {
                inner,
                registered: Vec::new(),
            })
        }

        /// Replace all registrations with the ones described by `settings`.
        pub fn register_all(&mut self, settings: &HotkeySettings) -> Vec<(HotkeyAction, Result<(), String>)> {
            for (action, hk) in self.registered.drain(..) {
                if let Err(e) = self.inner.unregister(hk) {
                    tracing::warn!("could not unregister {}: {e}", action.label());
                }
            }
            let mut results = Vec::with_capacity(HotkeyAction::ALL.len());
            for (action, planned) in plan_registrations(settings) {
                let result = match planned {
                    Err(msg) => Err(msg),
                    Ok(spec) => match to_hotkey(&spec) {
                        None => Err(format!("\"{}\" has no Windows key mapping.", spec.format())),
                        Some(hk) => match self.inner.register(hk) {
                            Ok(()) => {
                                self.registered.push((action, hk));
                                Ok(())
                            }
                            Err(e) => {
                                tracing::warn!("hotkey {} refused by the system: {e}", spec.format());
                                Err(format!("\"{}\" is in use by another application.", spec.format()))
                            }
                        },
                    },
                };
                results.push((action, result));
            }
            results
        }

        /// Drain pending key presses and return the actions triggered since the last call.
        pub fn poll(&mut self) -> Vec<HotkeyAction> {
            let mut fired = Vec::new();
            while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                if event.state != HotKeyState::Pressed {
                    continue;
                }
                if let Some((action, _)) = self.registered.iter().find(|(_, hk)| hk.id() == event.id) {
                    fired.push(*action);
                }
            }
            fired
        }
    }

    impl Drop for HotkeyManager {
        fn drop(&mut self) {
            for (_, hk) in self.registered.drain(..) {
                let _ = self.inner.unregister(hk);
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{HotkeyAction, plan_registrations};
    use crate::storage::config::HotkeySettings;

    pub struct HotkeyManager;

    impl HotkeyManager {
        pub fn new() -> Result<Self, String> {
            Ok(Self)
        }

        /// Validation still runs everywhere; valid bindings report that OS hotkeys need Windows.
        pub fn register_all(&mut self, settings: &HotkeySettings) -> Vec<(HotkeyAction, Result<(), String>)> {
            plan_registrations(settings)
                .into_iter()
                .map(|(action, planned)| {
                    (
                        action,
                        planned.and_then(|_| Err("Global hotkeys are only available on Windows.".to_string())),
                    )
                })
                .collect()
        }

        pub fn poll(&mut self) -> Vec<HotkeyAction> {
            Vec::new()
        }
    }
}

pub use platform::HotkeyManager;
#[cfg(windows)]
pub use platform::parse_hotkey;

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(crosshair: &str, overlay: &str, next: &str, prev: &str, auto: &str) -> HotkeySettings {
        HotkeySettings {
            toggle_crosshair: crosshair.into(),
            toggle_overlay: overlay.into(),
            next_preset: next.into(),
            prev_preset: prev.into(),
            toggle_auto_profiles: auto.into(),
        }
    }

    #[test]
    fn parses_common_combinations_and_normalizes() {
        assert_eq!(normalize_hotkey("Ctrl+Alt+F1").unwrap(), "Ctrl+Alt+F1");
        assert_eq!(normalize_hotkey("ctrl + alt + f1").unwrap(), "Ctrl+Alt+F1");
        assert_eq!(normalize_hotkey("Ctrl+Shift+Right").unwrap(), "Ctrl+Shift+Right");
        assert_eq!(normalize_hotkey("Control+shift+arrowleft").unwrap(), "Ctrl+Shift+Left");
        assert_eq!(normalize_hotkey("Shift+Win+r").unwrap(), "Shift+Win+R");
        assert_eq!(normalize_hotkey("alt+5").unwrap(), "Alt+5");
        assert_eq!(normalize_hotkey("Ctrl+Escape").unwrap(), "Ctrl+Esc");
        assert_eq!(normalize_hotkey("Ctrl+[").unwrap(), "Ctrl+[");
        assert_eq!(normalize_hotkey("Ctrl+OpenBracket").unwrap(), "Ctrl+[");
        assert_eq!(normalize_hotkey("Ctrl+Num5").unwrap(), "Ctrl+Numpad5");
    }

    #[test]
    fn function_keys_may_stand_alone() {
        assert_eq!(normalize_hotkey("F9").unwrap(), "F9");
        assert_eq!(normalize_hotkey("f24").unwrap(), "F24");
        assert_eq!(normalize_hotkey("F25"), Err(HotkeyError::UnknownKey("F25".into())));
    }

    #[test]
    fn rejects_bare_non_function_keys() {
        assert_eq!(parse_spec("A"), Err(HotkeyError::NeedsModifier));
        assert_eq!(parse_spec("Space"), Err(HotkeyError::NeedsModifier));
    }

    #[test]
    fn reports_clear_parse_errors() {
        assert_eq!(parse_spec("   "), Err(HotkeyError::Empty));
        assert_eq!(parse_spec("Ctrl+"), Err(HotkeyError::MissingKey));
        assert_eq!(parse_spec("Ctrl"), Err(HotkeyError::MissingKey));
        assert_eq!(parse_spec("Ctrl++F1"), Err(HotkeyError::EmptyPart));
        assert_eq!(parse_spec("Ctrl+Foo"), Err(HotkeyError::UnknownKey("Foo".into())));
        assert_eq!(
            parse_spec("Ctrl+A+B"),
            Err(HotkeyError::MultipleKeys("A".into(), "B".into()))
        );
        assert_eq!(
            parse_spec("Ctrl+A+Shift"),
            Err(HotkeyError::ModifierAfterKey("Shift".into()))
        );
        assert_eq!(
            parse_spec("Ctrl+Control+A"),
            Err(HotkeyError::RepeatedModifier("Control".into()))
        );
        assert_eq!(
            HotkeyError::UnknownKey("Foo".into()).to_string(),
            "\"Foo\" is not a recognized key"
        );
    }

    #[test]
    fn spec_format_is_stable_and_ordered() {
        let spec = parse_spec("Win+Shift+Alt+Ctrl+X").unwrap();
        assert_eq!(spec.format(), "Ctrl+Alt+Shift+Win+X");
        assert_eq!(parse_spec(&spec.format()).unwrap(), spec);
    }

    #[test]
    fn defaults_have_no_conflicts() {
        let plan = plan_registrations(&HotkeySettings::default());
        assert_eq!(plan.len(), HotkeyAction::ALL.len());
        assert!(plan.iter().all(|(_, r)| r.is_ok()), "{plan:?}");
    }

    #[test]
    fn detects_duplicates_within_our_own_set() {
        let s = settings(
            "Ctrl+Alt+F1",
            "ctrl+alt+f1",
            "Ctrl+Alt+Right",
            "Ctrl+Alt+Left",
            "Ctrl+Alt+F3",
        );
        let plan = plan_registrations(&s);
        assert!(plan[0].1.is_ok());
        assert_eq!(
            plan[1].1,
            Err("Already used by \"Toggle crosshair\" in this app.".to_string())
        );
        assert!(plan[2].1.is_ok());
    }

    #[test]
    fn reports_invalid_and_empty_bindings_per_action() {
        let s = settings("", "Ctrl+", "Ctrl+Alt+Right", "A", "Ctrl+Alt+F3");
        let plan = plan_registrations(&s);
        assert_eq!(plan[0].1, Err("no key combination is set".to_string()));
        assert_eq!(
            plan[1].1,
            Err("Invalid key combination \"Ctrl+\": add a main key after the modifiers, such as F1 or R.".to_string())
        );
        assert_eq!(
            plan[3].1,
            Err(
                "Invalid key combination \"A\": add Ctrl, Alt, Shift or Win; only F1 to F24 can be used on their own."
                    .to_string()
            )
        );
        assert!(plan[2].1.is_ok());
        assert!(plan[4].1.is_ok());
    }

    #[test]
    fn non_windows_stub_reports_platform_limit_for_valid_bindings() {
        let mut mgr = HotkeyManager::new().unwrap();
        let results = mgr.register_all(&HotkeySettings::default());
        #[cfg(not(windows))]
        assert!(results.iter().all(
            |(_, r)| r.as_ref().err().map(String::as_str) == Some("Global hotkeys are only available on Windows.")
        ));
        #[cfg(windows)]
        assert_eq!(results.len(), HotkeyAction::ALL.len());
        assert!(mgr.poll().is_empty());
    }

    #[test]
    fn action_binding_reads_matching_field() {
        let s = settings("A1", "A2", "A3", "A4", "A5");
        assert_eq!(HotkeyAction::ToggleCrosshair.binding(&s), "A1");
        assert_eq!(HotkeyAction::ToggleOverlay.binding(&s), "A2");
        assert_eq!(HotkeyAction::NextPreset.binding(&s), "A3");
        assert_eq!(HotkeyAction::PrevPreset.binding(&s), "A4");
        assert_eq!(HotkeyAction::ToggleAutoProfiles.binding(&s), "A5");
    }
}
