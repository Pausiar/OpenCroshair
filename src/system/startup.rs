//! Launch at Windows sign-in via the per-user `HKCU\...\Run` key.
//!
//! No administrator rights are needed: only the current user's Run key is touched.

use std::path::Path;

/// Registry value name under the Run key.
#[allow(dead_code)]
pub const VALUE_NAME: &str = "OpenCrosshair";

/// Value data written to the Run key: quoted exe path followed by ` --minimized`.
pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" --minimized", exe.display())
}

#[cfg(windows)]
mod imp {
    use super::run_value;
    use windows::Win32::Foundation::WIN32_ERROR;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
        RegQueryValueExW, RegSetValueExW,
    };
    use windows::core::w;

    const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    const ERROR_FILE_NOT_FOUND: u32 = 2;

    /// Closes the registry key on drop.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn open_run_key(access: u32) -> Result<Key, WIN32_ERROR> {
        let mut hkey = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                None,
                windows::Win32::System::Registry::REG_SAM_FLAGS(access),
                &mut hkey,
            )
        };
        if status.0 == 0 { Ok(Key(hkey)) } else { Err(status) }
    }

    pub fn is_enabled() -> bool {
        let Ok(key) = open_run_key(KEY_QUERY_VALUE.0) else {
            return false;
        };
        // Querying with no buffer only checks that the value exists.
        let status = unsafe { RegQueryValueExW(key.0, VALUE_NAME_W, None, None, None, None) };
        status.0 == 0
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        if enabled {
            let exe = std::env::current_exe().map_err(|e| format!("cannot locate executable: {e}"))?;
            let data = run_value(&exe);
            // REG_SZ data is UTF-16LE including the terminating NUL.
            let bytes: Vec<u8> = data
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(|u| u.to_le_bytes())
                .collect();
            let key = open_run_key(KEY_SET_VALUE.0).map_err(|e| format!("open Run key failed: {}", e.0))?;
            let status = unsafe { RegSetValueExW(key.0, VALUE_NAME_W, None, REG_SZ, Some(&bytes)) };
            if status.0 != 0 {
                return Err(format!("write Run value failed: error {}", status.0));
            }
            Ok(())
        } else {
            let key = open_run_key(KEY_SET_VALUE.0).map_err(|e| format!("open Run key failed: {}", e.0))?;
            let status = unsafe { RegDeleteValueW(key.0, VALUE_NAME_W) };
            // Already absent is the desired end state.
            if status.0 != 0 && status.0 != ERROR_FILE_NOT_FOUND {
                return Err(format!("delete Run value failed: error {}", status.0));
            }
            Ok(())
        }
    }

    const VALUE_NAME_W: windows::core::PCWSTR = w!("OpenCrosshair");
}

#[cfg(not(windows))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }

    pub fn set_enabled(_enabled: bool) -> Result<(), String> {
        Err("not supported".to_string())
    }
}

/// Whether the app is currently registered to start with Windows.
pub fn is_enabled() -> bool {
    imp::is_enabled()
}

/// Register (`true`) or unregister (`false`) the app for launch at sign-in.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    imp::set_enabled(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn run_value_quotes_path_and_adds_minimized_flag() {
        let exe = PathBuf::from(r"C:\Program Files\OpenCrosshair\opencrosshair.exe");
        assert_eq!(
            run_value(&exe),
            r#""C:\Program Files\OpenCrosshair\opencrosshair.exe" --minimized"#
        );
    }

    #[test]
    fn value_name_is_stable() {
        assert_eq!(VALUE_NAME, "OpenCrosshair");
    }
}
