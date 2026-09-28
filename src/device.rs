//! Finding the plugged-in FIDO2 key(s) and asking the authenticator what it
//! supports. None of this needs the PIN.

use anyhow::{bail, Context, Result};
use ctap_hid_fido2::{
    fidokey::{get_info::InfoOption, FidoKeyHid},
    get_fidokey_devices, Cfg, FidoKeyHidFactory, HidInfo,
};

use crate::io_util::read_line;

/// Finds all plugged-in FIDO2 keys and, if there's more than one, asks the
/// user which one to use. Opens and returns a handle to the chosen key.
pub fn select_device() -> Result<FidoKeyHid> {
    let mut devices = get_fidokey_devices();
    if devices.is_empty() {
        bail!("No FIDO2 security key found. Is it plugged in?");
    }

    let chosen: HidInfo = if devices.len() == 1 {
        devices.remove(0)
    } else {
        println!("Multiple FIDO2 keys found:");
        for (i, d) in devices.iter().enumerate() {
            println!("  [{}] {}", i + 1, describe_device(d));
        }
        loop {
            let input = read_line("Which key do you want to use? (number): ")?;
            match input.trim().parse::<usize>() {
                Ok(n) if n >= 1 && n <= devices.len() => break devices.remove(n - 1),
                _ => println!(
                    "Please enter a number between 1 and {}.",
                    devices.len()
                ),
            }
        }
    };

    FidoKeyHidFactory::create_by_params(std::slice::from_ref(&chosen.param), &Cfg::init())
        .context("Could not open the selected security key")
}

/// Short human-readable label for a device in the picker list.
fn describe_device(d: &HidInfo) -> String {
    if d.product_string.is_empty() {
        format!("VID:PID {:04x}:{:04x}", d.vid, d.pid)
    } else {
        format!("{} (VID:PID {:04x}:{:04x})", d.product_string, d.vid, d.pid)
    }
}

pub fn is_credential_management_supported(device: &FidoKeyHid) -> Result<bool> {
    Ok(device.enable_info_option(&InfoOption::CredMgmt)?.is_some()
        || device
            .enable_info_option(&InfoOption::CredentialMgmtPreview)?
            .is_some())
}

pub fn is_bio_enrollment_supported(device: &FidoKeyHid) -> Result<bool> {
    Ok(device.enable_info_option(&InfoOption::BioEnroll)?.is_some()
        || device.enable_info_option(&InfoOption::UvBioEnroll)?.is_some())
}