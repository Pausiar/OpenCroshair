//! Profile matching and CRUD logic. Pure: no I/O, no Win32.
use super::GameProfile;

/// Errors from validated profile edits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    #[error("Profile name cannot be empty.")]
    EmptyName,
    #[error("Executable must be a file name ending in .exe (for example cs2.exe).")]
    InvalidExe,
    #[error("\"{exe}\" is already used by the profile \"{profile}\".")]
    DuplicateExe { exe: String, profile: String },
    #[error("That profile no longer exists.")]
    NotFound,
}

/// Reduce a file name or full path to a trimmed, lowercase file name.
/// Accepts `C:\Games\CS2.exe`, `/opt/cs2.exe` and `CS2.EXE`. Returns "" for empty input.
pub fn normalize_exe(input: &str) -> String {
    let trimmed = input.trim().trim_matches('"').trim();
    let name = trimmed.rsplit(['\\', '/']).next().unwrap_or("").trim();
    name.to_ascii_lowercase()
}

/// Whether `exe` (already a file name) is a syntactically valid executable name.
fn is_valid_exe(normalized: &str) -> bool {
    normalized.len() > ".exe".len() && normalized.ends_with(".exe")
}

/// Find the first enabled profile whose executable matches `exe` (case-insensitive;
/// `exe` may be a file name or a full path). Disabled profiles and empty entries never match.
pub fn match_profile<'a>(profiles: &'a [GameProfile], exe: &str) -> Option<&'a GameProfile> {
    if normalize_exe(exe).is_empty() {
        return None;
    }
    profiles.iter().find(|p| p.matches_exe(exe))
}

/// Outcome of applying the auto-profile rules to a foreground change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A profile matched. `preset_id: None` means the profile does not change the crosshair.
    Profile {
        profile_id: String,
        preset_id: Option<String>,
    },
    /// No profile matches but one was active before: go back to the default preset.
    /// `preset_id: None` means no default is configured; the crosshair is left unchanged.
    Revert { preset_id: Option<String> },
    /// Nothing to do.
    Keep,
}

/// Decide what the crosshair should do after the foreground executable changed.
///
/// * A matching enabled profile always wins.
/// * If no profile matches, the default preset (`default_preset_id`) is used only when a game
///   profile was active before (`active_profile_id`), so that leaving the game restores the default.
///   Without a previously active profile the user's current crosshair is never overridden.
pub fn resolve_preset_id(
    profiles: &[GameProfile],
    exe: Option<&str>,
    default_preset_id: Option<&str>,
    active_profile_id: Option<&str>,
) -> Resolution {
    if let Some(profile) = exe.and_then(|e| match_profile(profiles, e)) {
        return Resolution::Profile {
            profile_id: profile.id.clone(),
            preset_id: profile.preset_id.clone(),
        };
    }
    if active_profile_id.is_some() {
        return Resolution::Revert {
            preset_id: default_preset_id.map(str::to_string),
        };
    }
    Resolution::Keep
}

/// Check name and executable and return the normalized `(name, exe)`.
fn validate(
    profiles: &[GameProfile],
    name: &str,
    exe: &str,
    editing_id: Option<&str>,
) -> Result<(String, String), ProfileError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProfileError::EmptyName);
    }
    let exe = normalize_exe(exe);
    if !is_valid_exe(&exe) {
        return Err(ProfileError::InvalidExe);
    }
    if let Some(other) = profiles
        .iter()
        .find(|p| Some(p.id.as_str()) != editing_id && normalize_exe(&p.exe) == exe)
    {
        return Err(ProfileError::DuplicateExe {
            exe,
            profile: other.name.clone(),
        });
    }
    Ok((name.to_string(), exe))
}

/// Add a validated profile and return its id.
pub fn add_profile(
    profiles: &mut Vec<GameProfile>,
    name: &str,
    exe: &str,
    preset_id: Option<String>,
) -> Result<String, ProfileError> {
    let (name, exe) = validate(profiles, name, exe, None)?;
    let profile = GameProfile {
        name,
        exe,
        preset_id,
        ..GameProfile::default()
    };
    let id = profile.id.clone();
    profiles.push(profile);
    Ok(id)
}

/// Update a profile in place. Duplicate checks ignore the profile being edited.
pub fn update_profile(
    profiles: &mut [GameProfile],
    id: &str,
    name: &str,
    exe: &str,
    preset_id: Option<String>,
    enabled: bool,
) -> Result<(), ProfileError> {
    if !profiles.iter().any(|p| p.id == id) {
        return Err(ProfileError::NotFound);
    }
    let (name, exe) = validate(profiles, name, exe, Some(id))?;
    if let Some(p) = profiles.iter_mut().find(|p| p.id == id) {
        p.name = name;
        p.exe = exe;
        p.preset_id = preset_id;
        p.enabled = enabled;
    }
    Ok(())
}

/// Remove a profile by id and return it.
pub fn remove_profile(profiles: &mut Vec<GameProfile>, id: &str) -> Result<GameProfile, ProfileError> {
    let idx = profiles.iter().position(|p| p.id == id).ok_or(ProfileError::NotFound)?;
    Ok(profiles.remove(idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, name: &str, exe: &str, preset: Option<&str>, enabled: bool) -> GameProfile {
        GameProfile {
            id: id.into(),
            name: name.into(),
            exe: exe.into(),
            preset_id: preset.map(str::to_string),
            enabled,
        }
    }

    #[test]
    fn normalize_reduces_paths_and_lowercases() {
        assert_eq!(normalize_exe(r"C:\Games\CS2\CS2.EXE"), "cs2.exe");
        assert_eq!(normalize_exe("/usr/bin/Game.exe"), "game.exe");
        assert_eq!(normalize_exe("  \"Valorant.exe\"  "), "valorant.exe");
        assert_eq!(normalize_exe("cs2.exe"), "cs2.exe");
        assert_eq!(normalize_exe("   "), "");
        assert_eq!(normalize_exe(r"C:\Games\"), "");
    }

    #[test]
    fn match_is_case_insensitive_and_accepts_paths() {
        let profiles = vec![profile("1", "CS2", "CS2.exe", Some("p1"), true)];
        assert_eq!(match_profile(&profiles, "cs2.exe").map(|p| p.id.as_str()), Some("1"));
        assert_eq!(
            match_profile(&profiles, r"D:\SteamLibrary\cs2.EXE").map(|p| p.id.as_str()),
            Some("1")
        );
        assert!(match_profile(&profiles, "cs.exe").is_none());
        assert!(match_profile(&profiles, "").is_none());
    }

    #[test]
    fn disabled_and_empty_profiles_never_match() {
        let profiles = vec![
            profile("off", "Off", "off.exe", None, false),
            profile("empty", "Empty", "  ", None, true),
        ];
        assert!(match_profile(&profiles, "off.exe").is_none());
        assert!(match_profile(&profiles, "").is_none());
        assert!(match_profile(&profiles, " ").is_none());
    }

    #[test]
    fn resolve_prefers_matching_profile() {
        let profiles = vec![profile("g", "Game", "game.exe", Some("gp"), true)];
        assert_eq!(
            resolve_preset_id(&profiles, Some("GAME.exe"), Some("default"), None),
            Resolution::Profile {
                profile_id: "g".into(),
                preset_id: Some("gp".into())
            }
        );
    }

    #[test]
    fn resolve_reverts_to_default_only_after_a_profile_was_active() {
        let profiles = vec![profile("g", "Game", "game.exe", Some("gp"), true)];
        // Previously active profile, now nothing matches -> default.
        assert_eq!(
            resolve_preset_id(&profiles, Some("desktop.exe"), Some("default"), Some("g")),
            Resolution::Revert {
                preset_id: Some("default".into())
            }
        );
        // Previously active, no default configured -> revert without a preset.
        assert_eq!(
            resolve_preset_id(&profiles, None, None, Some("g")),
            Resolution::Revert { preset_id: None }
        );
        // Nothing active before -> the user's crosshair is never overridden.
        assert_eq!(
            resolve_preset_id(&profiles, Some("desktop.exe"), Some("default"), None),
            Resolution::Keep
        );
    }

    #[test]
    fn profile_without_preset_is_still_reported_as_matched() {
        let profiles = vec![profile("g", "Game", "game.exe", None, true)];
        assert_eq!(
            resolve_preset_id(&profiles, Some("game.exe"), Some("default"), None),
            Resolution::Profile {
                profile_id: "g".into(),
                preset_id: None
            }
        );
    }

    #[test]
    fn add_validates_name_and_exe() {
        let mut profiles = Vec::new();
        assert_eq!(
            add_profile(&mut profiles, "  ", "a.exe", None),
            Err(ProfileError::EmptyName)
        );
        assert_eq!(
            add_profile(&mut profiles, "A", "a.dll", None),
            Err(ProfileError::InvalidExe)
        );
        assert_eq!(
            add_profile(&mut profiles, "A", ".exe", None),
            Err(ProfileError::InvalidExe)
        );
        assert_eq!(add_profile(&mut profiles, "A", "", None), Err(ProfileError::InvalidExe));
        let id = add_profile(&mut profiles, "  Counter-Strike 2 ", r"C:\CS2\CS2.EXE", None).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, id);
        assert_eq!(profiles[0].name, "Counter-Strike 2");
        assert_eq!(profiles[0].exe, "cs2.exe");
        assert!(profiles[0].enabled);
    }

    #[test]
    fn duplicate_exe_is_rejected_case_insensitively() {
        let mut profiles = Vec::new();
        add_profile(&mut profiles, "CS2", "cs2.exe", None).unwrap();
        let err = add_profile(&mut profiles, "Other", r"D:\x\CS2.EXE", None).unwrap_err();
        assert_eq!(
            err,
            ProfileError::DuplicateExe {
                exe: "cs2.exe".into(),
                profile: "CS2".into()
            }
        );
    }

    #[test]
    fn update_ignores_own_exe_for_duplicate_check_and_rejects_others() {
        let mut profiles = Vec::new();
        let a = add_profile(&mut profiles, "A", "a.exe", None).unwrap();
        let b = add_profile(&mut profiles, "B", "b.exe", None).unwrap();
        update_profile(&mut profiles, &a, "A2", "A.EXE", Some("p".into()), false).unwrap();
        assert_eq!(profiles[0].name, "A2");
        assert!(!profiles[0].enabled);
        assert_eq!(profiles[0].preset_id.as_deref(), Some("p"));
        assert!(matches!(
            update_profile(&mut profiles, &a, "A", "b.exe", None, true),
            Err(ProfileError::DuplicateExe { .. })
        ));
        assert_eq!(
            update_profile(&mut profiles, "missing", "X", "x.exe", None, true),
            Err(ProfileError::NotFound)
        );
        assert!(update_profile(&mut profiles, &b, "", "b.exe", None, true).is_err());
    }

    #[test]
    fn remove_returns_profile_or_not_found() {
        let mut profiles = Vec::new();
        let a = add_profile(&mut profiles, "A", "a.exe", None).unwrap();
        let removed = remove_profile(&mut profiles, &a).unwrap();
        assert_eq!(removed.exe, "a.exe");
        assert!(profiles.is_empty());
        assert_eq!(remove_profile(&mut profiles, &a), Err(ProfileError::NotFound));
    }
}
