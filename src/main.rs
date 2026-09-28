//! fido2-cred-manager
//!
//! An interactive CLI tool to manage a FIDO2 security key (e.g. a YubiKey):
//!
//! - list/delete discoverable credentials ("resident keys") via CTAP2's
//!   authenticatorCredentialManagement command
//! - view PIN retry count, set an initial PIN, change the PIN
//! - manage enrolled fingerprints on keys that have a sensor (bio enrollment)
//! - works with more than one key plugged in at once (you pick which one)
//!
//! IMPORTANT: this tool only talks to the plugged-in hardware key over USB
//! HID. It never touches any server, cloud account, or online service - it
//! only sees what the key itself reports.

mod bio;
mod creds;
mod device;
mod io_util;
mod pin;
mod platform;
mod text;

use anyhow::{bail, Result};
use ctap_hid_fido2::fidokey::get_info::InfoOption;

use crate::bio::run_bio_menu;
use crate::creds::run_credentials_menu;
use crate::device::{is_bio_enrollment_supported, is_credential_management_supported, select_device};
use crate::io_util::read_line;
use crate::pin::{confirm_and_read_pin, run_change_pin, set_initial_pin};
use crate::platform::{ensure_elevated, pause_if_own_console, Elevation};

fn main() {
    match ensure_elevated() {
        Elevation::Ok => {}
        Elevation::Relaunched => return,
        Elevation::Failed => {
            pause_if_own_console();
            std::process::exit(1);
        }
    }

    let result = run();

    // Errors are printed here (instead of returning them from `main`) so
    // that we can still pause afterwards. Without the pause, a double-
    // clicked .exe closes its console window instantly and the message
    // would vanish before anyone can read it.
    if let Err(e) = &result {
        eprintln!("Error: {:?}", e);
    }

    pause_if_own_console();

    if result.is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    println!("fido2-cred-manager - manage a FIDO2 security key\n");

    let device = select_device()?;

    // These capability checks need no PIN, so we can do them up front and
    // use them to decide which menu entries even make sense to offer.
    let cred_mgmt_supported = is_credential_management_supported(&device)?;
    let bio_supported = is_bio_enrollment_supported(&device)?;
    let client_pin_state = device.enable_info_option(&InfoOption::ClientPin)?;

    // `mut` because a successful "change PIN" mid-session updates this so
    // the rest of the session keeps using the new PIN without re-prompting.
    let mut pin = match client_pin_state {
        None => {
            bail!(
                "This authenticator has no PIN capability at all - there's nothing \
                 this tool can do with it."
            );
        }
        Some(false) => {
            println!("No PIN is set on this key yet.");
            set_initial_pin(&device)?
        }
        Some(true) => match confirm_and_read_pin(&device)? {
            Some(pin) => pin,
            None => {
                println!("Aborted.");
                return Ok(());
            }
        },
    };

    loop {
        println!("\nWhat do you want to do?");
        let mut options: Vec<(&str, MenuAction)> = Vec::new();
        if cred_mgmt_supported {
            options.push(("Manage credentials (list/delete)", MenuAction::Credentials));
        }
        options.push(("Change PIN", MenuAction::ChangePin));
        if bio_supported {
            options.push(("Manage fingerprints", MenuAction::Bio));
        }
        options.push(("Quit", MenuAction::Quit));

        for (i, (label, _)) in options.iter().enumerate() {
            println!("  [{}] {}", i + 1, label);
        }

        let choice = read_line("> ")?;
        let idx = match choice.trim().parse::<usize>() {
            Ok(n) if n >= 1 && n <= options.len() => n - 1,
            _ => {
                println!("Please enter a number between 1 and {}.", options.len());
                continue;
            }
        };

        match options[idx].1 {
            MenuAction::Credentials => {
                if let Err(e) = run_credentials_menu(&device, &pin) {
                    eprintln!("Error: {:?}", e);
                }
            }
            MenuAction::ChangePin => match run_change_pin(&device, &pin) {
                Ok(Some(new_pin)) => {
                    pin = new_pin;
                    println!("PIN changed. Using the new PIN for the rest of this session.");
                }
                Ok(None) => println!("Aborted, PIN unchanged."),
                Err(e) => eprintln!("Error: {:?}", e),
            },
            MenuAction::Bio => {
                if let Err(e) = run_bio_menu(&device, &pin) {
                    eprintln!("Error: {:?}", e);
                }
            }
            MenuAction::Quit => break,
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum MenuAction {
    Credentials,
    ChangePin,
    Bio,
    Quit,
}