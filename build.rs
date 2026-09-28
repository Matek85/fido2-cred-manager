// Build script: embeds a Windows application manifest into the compiled
// .exe that requests administrator privileges (UAC elevation prompt).
//
// This is needed because the tool needs raw USB HID access to the FIDO2
// security key, and Windows blocks that for non-elevated processes since
// Windows 10 1903 (see README.md). Without this manifest, double-clicking
// the .exe would just silently fail to find the key; with it, Windows
// shows the UAC prompt automatically and the process starts elevated.
//
// On non-Windows hosts this script does nothing (the `embed-manifest`
// crate itself builds fine everywhere; only the actual embedding step is
// skipped when CARGO_CFG_WINDOWS isn't set).

use embed_manifest::{embed_manifest, manifest::ExecutionLevel, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(
            new_manifest("fido2-cred-manager").requested_execution_level(
                ExecutionLevel::RequireAdministrator,
            ),
        )
        .expect("unable to embed Windows manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}