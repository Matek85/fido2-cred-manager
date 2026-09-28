//! PIN handling: retry counter with lockout warning, setting an initial PIN,
//! and changing the PIN.

use anyhow::{Context, Result};
use ctap_hid_fido2::fidokey::FidoKeyHid;

use crate::io_util::read_line;

/// Shows the remaining PIN attempts, and if 2 or fewer remain, requires an
/// extra explicit "yes" before proceeding - getting the PIN wrong at that
/// point risks a full lockout, which can only be undone by a factory reset
/// that erases everything on the key. Returns None if the user backs out.
pub fn confirm_and_read_pin(device: &FidoKeyHid) -> Result<Option<String>> {
    let retries = device
        .get_pin_retries()
        .context("Failed to read the PIN retry count")?;
    println!("PIN attempts remaining: {}", retries);

    if retries <= 2 {
        println!(
            "\n/!\\ Only {} attempt(s) left before this key locks itself!",
            retries
        );
        println!(
            "A locked key can only be recovered with a full factory reset, \
             which erases every credential and fingerprint on it."
        );
        let answer = read_line("Do you REALLY want to use this PIN now? Type 'yes' to continue: ")?;
        if answer.trim() != "yes" {
            return Ok(None);
        }
    }

    let pin = rpassword::prompt_password("FIDO2 PIN: ").context("Failed to read PIN")?;
    Ok(Some(pin))
}

/// Sets a PIN on a key that doesn't have one yet. Asks twice to catch
/// typos, since (unlike changing a PIN) there's no existing PIN to notice
/// a mismatch against later.
pub fn set_initial_pin(device: &FidoKeyHid) -> Result<String> {
    loop {
        let pin1 = rpassword::prompt_password("Choose a new PIN: ").context("Failed to read PIN")?;
        let pin2 = rpassword::prompt_password("Repeat the new PIN: ").context("Failed to read PIN")?;
        if pin1 != pin2 {
            println!("The two entries didn't match, please try again.\n");
            continue;
        }
        device
            .set_new_pin(&pin1)
            .context("Failed to set the PIN on the key")?;
        println!("PIN set successfully.");
        return Ok(pin1);
    }
}

/// Changes the PIN. Returns the new PIN on success (the caller then keeps
/// using it for the rest of the session), or None if the user backed out.
pub fn run_change_pin(device: &FidoKeyHid, current_pin: &str) -> Result<Option<String>> {
    println!("\nChanging PIN (press Enter with an empty new PIN to cancel).");
    loop {
        let new1 = rpassword::prompt_password("New PIN: ").context("Failed to read PIN")?;
        if new1.is_empty() {
            return Ok(None);
        }
        let new2 = rpassword::prompt_password("Repeat new PIN: ").context("Failed to read PIN")?;
        if new1 != new2 {
            println!("The two entries didn't match, please try again.\n");
            continue;
        }
        device
            .change_pin(current_pin, &new1)
            .context("Failed to change the PIN (is the current PIN correct?)")?;
        println!("PIN changed successfully.");
        return Ok(Some(new1));
    }
}