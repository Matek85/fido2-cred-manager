//! Fingerprint (bio enrollment) management: list, enroll, rename, delete.
//!
//! NOTE: this whole module has not been tested against real hardware (no
//! fingerprint-capable FIDO2 key was available while building it). The
//! calls are taken directly from the ctap-hid-fido2 crate's own source, but
//! please treat this feature with extra caution the first time you use it,
//! and keep a note of what you enroll/delete in case something looks off.

use anyhow::{Context, Result};
use ctap_hid_fido2::{
    fidokey::{bio::TemplateInfo, FidoKeyHid},
    util,
};

use crate::io_util::{pick_index, read_line};
use crate::text::decode_html_entities;

pub fn run_bio_menu(device: &FidoKeyHid, pin: &str) -> Result<()> {
    loop {
        // Some authenticators return an error (instead of an empty list)
        // when zero fingerprints are enrolled yet, mirroring a quirk also
        // seen in credential enumeration. We treat that case as "none yet"
        // rather than a hard failure, since it's the more likely reading -
        // but this is unverified against real hardware.
        let enrollments = match device.bio_enrollment_enumerate_enrollments(pin) {
            Ok(list) => list,
            Err(e) => {
                println!(
                    "(Could not list fingerprints, assuming none are enrolled yet: {:?})",
                    e
                );
                Vec::new()
            }
        };

        println!("\nEnrolled fingerprints:");
        if enrollments.is_empty() {
            println!("  (none)");
        } else {
            for (i, t) in enrollments.iter().enumerate() {
                let name = t
                    .template_friendly_name
                    .clone()
                    .unwrap_or_else(|| "(unnamed)".to_string());
                println!(
                    "  [{}] {} ({})",
                    i + 1,
                    decode_html_entities(&name),
                    util::to_hex_str(&t.template_id)
                );
            }
        }

        println!("\n  [n] Enroll a new fingerprint");
        println!("  [r] Rename a fingerprint");
        println!("  [d] Delete a fingerprint");
        println!("  [q] Back to main menu");
        let choice = read_line("> ")?;
        match choice.trim() {
            "n" => enroll_fingerprint(device, pin)?,
            "r" => rename_fingerprint(device, &enrollments, pin)?,
            "d" => delete_fingerprint(device, &enrollments, pin)?,
            "q" | "" => return Ok(()),
            _ => println!("Please enter n, r, d, or q."),
        }
    }
}

fn enroll_fingerprint(device: &FidoKeyHid, pin: &str) -> Result<()> {
    println!("Touch the sensor repeatedly until enrollment finishes; follow the key's own feedback (e.g. its LED).");
    let (status1, mut status2) = device
        .bio_enrollment_begin(pin, Some(10_000))
        .context("Failed to start fingerprint enrollment")?;
    println!(
        "{} ({} sample(s) remaining)",
        status2.message, status2.remaining_samples
    );

    while !status2.is_finish {
        status2 = device
            .bio_enrollment_next(&status1, Some(10_000))
            .context("Failed during fingerprint enrollment")?;
        println!(
            "{} ({} sample(s) remaining)",
            status2.message, status2.remaining_samples
        );
    }
    println!("Fingerprint enrolled.");

    let name = read_line("Friendly name for this fingerprint (optional, Enter to skip): ")?;
    let name = name.trim();
    if !name.is_empty() {
        device
            .bio_enrollment_set_friendly_name(pin, &status1.template_id, name)
            .context("Fingerprint was enrolled, but setting its name failed")?;
    }
    Ok(())
}

fn rename_fingerprint(device: &FidoKeyHid, enrollments: &[TemplateInfo], pin: &str) -> Result<()> {
    if enrollments.is_empty() {
        println!("No fingerprints enrolled.");
        return Ok(());
    }
    let idx = match pick_index("Which fingerprint? (number, Enter to cancel): ", enrollments.len())? {
        Some(i) => i,
        None => return Ok(()),
    };
    let new_name = read_line("New friendly name: ")?;
    let new_name = new_name.trim();
    if new_name.is_empty() {
        println!("Cancelled.");
        return Ok(());
    }
    device
        .bio_enrollment_set_friendly_name(pin, &enrollments[idx].template_id, new_name)
        .context("Failed to rename fingerprint")?;
    println!("Renamed.");
    Ok(())
}

fn delete_fingerprint(device: &FidoKeyHid, enrollments: &[TemplateInfo], pin: &str) -> Result<()> {
    if enrollments.is_empty() {
        println!("No fingerprints enrolled.");
        return Ok(());
    }
    let idx = match pick_index(
        "Which fingerprint to delete? (number, Enter to cancel): ",
        enrollments.len(),
    )? {
        Some(i) => i,
        None => return Ok(()),
    };
    let confirm = read_line(&format!(
        "Really delete fingerprint {}? Type 'yes' to confirm: ",
        idx + 1
    ))?;
    if confirm.trim() != "yes" {
        println!("Cancelled.");
        return Ok(());
    }
    device
        .bio_enrollment_remove(pin, &enrollments[idx].template_id)
        .context("Failed to delete fingerprint")?;
    println!("Deleted.");
    Ok(())
}