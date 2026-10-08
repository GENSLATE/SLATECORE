//! Generates the Tauri context and an `allow-<command>` permission per IPC command, so each
//! window's capability lists exactly the commands it may call (the tray menu gets a subset).

/// Every command in `lib.rs`'s `generate_handler!` (keep in step; a test checks it).
const COMMANDS: &[&str] = &["get_context"];

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))?;
    Ok(())
}
