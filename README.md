# fido2-cred-manager

![Rust](https://img.shields.io/badge/rust-stable-orange)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-blue)

Interactive command-line tool to manage a FIDO2 security key (e.g. a
YubiKey) directly over USB HID (CTAP2) — credentials, PIN, and
fingerprints, without the data ever leaving the key.

> **Security note:** This tool only talks to the plugged-in hardware key
> over USB HID. It has no connection to any online service, server, or
> cloud account (including Entra ID/Azure) — it only sees what the key
> itself reports.

## Contents

- [Features](#features)
- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [Administrator rights on Windows](#administrator-rights-on-windows)
- [Project structure](#project-structure)
- [Known limitations](#known-limitations)
- [How this was built](#how-this-was-built)
- [Contributing](#contributing)
- [License](#license)

## Features

- ✅ **Manage credentials** — list and selectively delete stored
  "discoverable credentials" (resident keys)
  (`authenticatorCredentialManagement`)
- ✅ **PIN management** — show remaining PIN attempts (with an extra
  confirmation once ≤2 attempts are left), set an initial PIN, change PIN
- ⚠️ **Fingerprint management** — enroll, rename, delete
  (`authenticatorBioEnrollment`) for keys with a sensor (e.g. YubiKey Bio)
  — **untested**, see [Known limitations](#known-limitations)
- ✅ **Multiple keys** — also works with several security keys plugged in
  at once (pick one from a menu)
- ✅ **Clean table output** — dynamic column widths, HTML entity decoding
  for accented characters (`&#246;` → `ö`)

## Requirements

### Build requirements

Only needed on the machine where you compile the tool:

- Rust/Cargo (a current toolchain, e.g. via [rustup](https://rustup.rs))
- Additionally, depending on the OS, since `hidapi` (a transitive
  dependency) has a small C component:
  - **Windows**: the MSVC toolchain (default target
    `x86_64-pc-windows-msvc`) plus the Visual Studio Build Tools ("Desktop
    development with C++"). No `libudev`/`pkg-config` needed — that's only
    relevant on Linux, since Windows uses the OS's native HID API.
  - **Linux**: `libudev-dev` and `pkg-config`, e.g.
    `sudo apt install libudev-dev pkg-config`. These are needed **only to
    compile**: `pkg-config` and the development files are used by the
    build script to locate and link `libudev`.
  - **macOS**: no extra packages needed (the Xcode Command Line Tools are
    enough)

### Runtime requirements

Needed on every machine where the finished binary is run:

- At least one FIDO2 key with PIN support. Credential Management and Bio
  Enrollment are optional CTAP2.1 features — not every key supports them;
  the tool detects this automatically and only shows the corresponding
  menu entries when the plugged-in key actually offers them.
- **Windows**: administrator rights (the tool requests them via a UAC
  prompt, see [Administrator rights on Windows](#administrator-rights-on-windows)).
  Nothing else has to be installed, except that a machine without the
  Microsoft Visual C++ Redistributable may fail to start the `.exe` (the
  default Rust MSVC build links the C runtime dynamically); most Windows
  systems already have it.
- **Linux**: the regular `libudev` runtime library (`libudev.so.1`, part of
  systemd/udev), which is present on practically every desktop
  installation — the `-dev` package and `pkg-config` are **not** needed to
  run the binary. Your user also needs read/write access to the key's
  `/dev/hidraw*` device. Modern distributions ship udev rules that grant
  this to the logged-in user; on older systems you may need to install the
  package providing FIDO/U2F udev rules, or run the tool with `sudo`.
  Without access, the tool reports that no key was found even though one
  is plugged in.
- **macOS**: no extra packages needed.

## Installation

```bash
git clone <repo-url>
cd fido2-cred-manager
cargo build --release
```

The finished binary ends up at `target/release/fido2-cred-manager` (or
`.exe` on Windows).

## Usage

```bash
./target/release/fido2-cred-manager
```

On Windows, double-clicking the `.exe` automatically triggers the UAC
elevation prompt (see [below](#administrator-rights-on-windows)). When the
tool owns its console window (double-click), it waits for Enter before
exiting so that messages and errors stay readable; when started from an
already open terminal, it exits immediately as usual.

Example session:

```
fido2-cred-manager - manage a FIDO2 security key

PIN attempts remaining: 8
FIDO2 PIN: ********

What do you want to do?
  [1] Manage credentials (list/delete)
  [2] Change PIN
  [3] Manage fingerprints
  [4] Quit
> 1

3 discoverable credential(s) stored, 22 more possible.

#   Service (RP ID)            User                              Display name   Credential ID (short)
----------------------------------------------------------------------------------------------------
1   github.com                 max.mustermann                    Max            a1b2c3d4e5f6a7b8...
2   login.microsoftonline.com  max@contoso.onmicrosoft.com        Max Mustermann 9f8e7d6c5b4a3c2d...
3   contoso.com                max.mustermann@contoso.com          Max            0011223344556677...

Which entries do you want to delete?
Enter comma-separated numbers (e.g. 1,3,5), or just press Enter to go back.
> 2

About to permanently delete:
  [2] login.microsoftonline.com (max@contoso.onmicrosoft.com / Max Mustermann)

Type 'yes' to confirm: yes
Deleted: login.microsoftonline.com (max@contoso.onmicrosoft.com / Max Mustermann)

Done.
```

Quick overview of the menu entries:

- **Manage credentials**: a numbered table of all stored entries
  (service/RP ID, username, display name, shortened credential ID); enter
  comma-separated numbers to delete, then confirm with `yes`. Depending on
  the key, each deletion requires a touch on the sensor.
- **Change PIN**: enter the new PIN twice; it's then used automatically
  for the rest of the session. If no PIN is set yet, the tool asks for one
  right at startup.
- **Manage fingerprints**: enroll (touch the sensor repeatedly as
  prompted), rename, delete.

## Administrator rights on Windows

Windows has blocked raw HID access to FIDO2 devices for non-elevated
processes since Windows 10 1903 — this is a deliberate OS security policy
and can't be worked around (even the Windows WebAuthn API doesn't pass
through the CTAP2 credential-management request needed here for external
keys).

Because of that, the `.exe` has a Windows manifest embedded (`build.rs` +
the `embed-manifest` crate) that requests `requireAdministrator`. Double-
clicking it therefore automatically triggers the UAC elevation prompt — no
need to manually start a terminal as Administrator. On Linux/macOS the
manifest has no effect (there, `build.rs` is simply a no-op).

As an independent safety net, the program also checks at startup whether it
is actually elevated. If it is not (for example because it was built
without the manifest), it asks Windows to start a fresh elevated copy of
itself, which shows the UAC prompt, and exits. If the prompt is declined,
it prints an error instead of continuing with a confusing "no key found".

Note for development: because of the manifest, `cargo run` from a
non-elevated terminal fails with "The requested operation requires
elevation (os error 740)". Either run `cargo run` from an elevated terminal
or start the built `.exe` directly.

## Project structure

```
fido2-cred-manager/
├── Cargo.toml
├── build.rs          # embeds the Windows manifest (must stay in the project root!)
└── src/
    ├── main.rs       # entry point, main menu
    ├── platform.rs   # Windows-only: UAC elevation, keep console window open (all `unsafe` code)
    ├── device.rs     # find/select the key, capability checks
    ├── pin.rs        # PIN retries + lockout warning, set/change PIN
    ├── creds.rs      # list and delete discoverable credentials, table output
    ├── bio.rs        # fingerprint management (untested, see below)
    ├── text.rs       # display helpers: truncation, HTML entity decoding
    └── io_util.rs    # terminal input helpers
```

Note that `build.rs` is not a normal source file: Cargo only picks it up in
the project root next to `Cargo.toml`. Placed in `src/` it is silently
ignored, and the Windows manifest would not be embedded.

## Known limitations

**No automatic detection of "outdated" entries.** FIDO2/CTAP does not
store a creation date for resident keys. The tool shows the service,
username, and display name so you can decide yourself which entries
belong to services you no longer use.

**Fingerprint management is untested.** The bio enrollment functions were
not tested against real hardware while building this tool (no
fingerprint-capable key was available). The calls used are taken directly
from the `ctap-hid-fido2` crate's source and have been type-checked, but
actual behavior on a real sensor-equipped key is unverified. Test
carefully before relying on it in production — especially deleting a
fingerprint. Feedback/issues on this are welcome.

**No backup or migration possible.** Neither credentials nor fingerprints
can be exported or transferred to another key. This isn't a limitation of
this tool but a design property of FIDO2: the private key and biometric
reference data never leave the security chip, and CTAP2 defines no
export/restore command. Deletion is always final — the only "backup"
strategy in FIDO2 is registering important services with a second,
independent key.

## How this was built

This tool was built with Claude Sonnet 5 (Anthropic). The API calls were
checked against the source code of the `ctap-hid-fido2` crate, and the
code was compiled and linted in a test environment. It was **not** tested
against every kind of hardware, and the fingerprint management in
particular has never run on a real sensor-equipped key (see
[Known limitations](#known-limitations)). As with any tool that deletes
data from a security key, try it with unimportant credentials first.

## Contributing

Issues and pull requests are welcome — especially feedback on fingerprint
management with real hardware (see above), since that couldn't be tested
so far.

## License

Not finalized yet — a permissive license like MIT would fit a tool like
this. Until a `LICENSE` file is added to the repo, it's "all rights
reserved".