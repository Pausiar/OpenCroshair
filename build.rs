//! Build script: embeds the app icon and a Windows application manifest into the .exe.
//!
//! Only does work when the *target* OS is Windows. Resource compilation failures (for example
//! a missing `windres` when cross-compiling from Linux) are reported as cargo warnings and do
//! not fail the build.

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 / 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let icon = manifest_dir.join("assets").join("icons").join("icon.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_manifest(MANIFEST);
    if icon.exists() {
        res.set_icon(&icon.to_string_lossy());
    } else {
        println!(
            "cargo:warning=icon not found at {}; exe will have no icon",
            icon.display()
        );
    }

    if let Err(e) = res.compile() {
        println!("cargo:warning=failed to embed Windows resources (icon/manifest): {e}");
    }
}
