//! Credential management: list the discoverable credentials (resident keys)
//! stored on the key and delete selected ones.

use anyhow::{bail, Context, Result};
use ctap_hid_fido2::{
    fidokey::{
        credential_management::credential_management_params::Credential, FidoKeyHid,
    },
    util,
};

use crate::io_util::read_line;
use crate::text::{decode_html_entities, truncate};

/// One row in the flattened list shown to the user: which relying party
/// (service) a credential belongs to, plus the credential itself.
struct Entry {
    rp_id: String,
    credential: Credential,
}

pub fn run_credentials_menu(device: &FidoKeyHid, pin: &str) -> Result<()> {
    let counts = device
        .credential_management_get_creds_metadata(Some(pin))
        .context("Failed to read credential metadata")?;
    println!(
        "\n{} discoverable credential(s) stored, {} more possible.\n",
        counts.existing_resident_credentials_count,
        counts.max_possible_remaining_resident_credentials_count
    );

    if counts.existing_resident_credentials_count == 0 {
        println!("Nothing to do.");
        return Ok(());
    }

    let rps = device
        .credential_management_enumerate_rps(Some(pin))
        .context("Failed to enumerate relying parties")?;

    let mut entries: Vec<Entry> = Vec::new();
    for rp in rps {
        let creds = device
            .credential_management_enumerate_credentials(Some(pin), &rp.rpid_hash)
            .with_context(|| {
                format!(
                    "Failed to enumerate credentials for {}",
                    rp.public_key_credential_rp_entity.id
                )
            })?;
        for credential in creds {
            entries.push(Entry {
                rp_id: rp.public_key_credential_rp_entity.id.clone(),
                credential,
            });
        }
    }

    print_entries(&entries);

    println!("\nWhich entries do you want to delete?");
    println!("Enter comma-separated numbers (e.g. 1,3,5), or just press Enter to go back.");
    let selection = read_line("> ")?;
    let selection = selection.trim();
    if selection.is_empty() {
        return Ok(());
    }

    let indices = parse_indices(selection, entries.len())?;
    if indices.is_empty() {
        println!("No valid entries selected, nothing deleted.");
        return Ok(());
    }

    println!("\nAbout to permanently delete:");
    for &i in &indices {
        println!("  [{}] {}", i + 1, describe(&entries[i]));
    }
    let confirm = read_line("\nType 'yes' to confirm: ")?;
    if confirm.trim() != "yes" {
        println!("Aborted, nothing deleted.");
        return Ok(());
    }

    let mut failures = 0;
    for &i in &indices {
        let entry = &entries[i];
        let descriptor = entry.credential.public_key_credential_descriptor.clone();
        match device.credential_management_delete_credential(Some(pin), descriptor) {
            Ok(()) => println!("Deleted: {}", describe(entry)),
            Err(e) => {
                failures += 1;
                eprintln!("Failed to delete {}: {:?}", describe(entry), e);
            }
        }
    }

    if failures > 0 {
        bail!("{} entr(y/ies) could not be deleted, see errors above.", failures);
    }

    println!("\nDone.");
    Ok(())
}

/// One already-formatted (but not yet padded) row of the table.
struct Row {
    num: String,
    rp: String,
    user: String,
    display: String,
    id: String,
}

/// Prints a numbered table of all discoverable credentials found on the key.
///
/// Column widths are computed from the actual content instead of being
/// fixed, so the table stays aligned regardless of how long a service name,
/// username, or display name is. The username column is never truncated
/// (real-world usernames, e.g. .onmicrosoft.com UPNs, can be long); service
/// name and display name are capped and truncated with "…" so one very long
/// value can't blow up the whole table. The full data is unaffected either
/// way, this is purely a display concern.
fn print_entries(entries: &[Entry]) {
    const MAX_RP_WIDTH: usize = 32;
    const MAX_DISPLAY_WIDTH: usize = 24;
    const ID_DISPLAY_CHARS: usize = 16; // 16 hex chars = first 8 bytes of the credential ID

    let rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let user = &entry.credential.public_key_credential_user_entity;
            let id_hex = util::to_hex_str(&entry.credential.public_key_credential_descriptor.id);
            let short_id: String = id_hex.chars().take(ID_DISPLAY_CHARS).collect();
            Row {
                num: (i + 1).to_string(),
                rp: truncate(&decode_html_entities(&entry.rp_id), MAX_RP_WIDTH),
                user: decode_html_entities(&user.name),
                display: truncate(&decode_html_entities(&user.display_name), MAX_DISPLAY_WIDTH),
                id: format!("{}...", short_id),
            }
        })
        .collect();

    // Each column's width is the widest value it actually needs (header
    // included), so short content doesn't waste space and long content
    // still lines up.
    let col_width = |header: &str, get: fn(&Row) -> &str| -> usize {
        rows.iter()
            .map(|r| get(r).chars().count())
            .max()
            .unwrap_or(0)
            .max(header.chars().count())
    };
    let num_w = col_width("#", |r| &r.num);
    let rp_w = col_width("Service (RP ID)", |r| &r.rp);
    let user_w = col_width("User", |r| &r.user);
    let display_w = col_width("Display name", |r| &r.display);
    let id_w = col_width("Credential ID (short)", |r| &r.id);

    println!(
        "{:<num_w$} {:<rp_w$} {:<user_w$} {:<display_w$} {:<id_w$}",
        "#",
        "Service (RP ID)",
        "User",
        "Display name",
        "Credential ID (short)",
    );
    println!("{}", "-".repeat(num_w + rp_w + user_w + display_w + id_w + 4));
    for r in &rows {
        println!(
            "{:<num_w$} {:<rp_w$} {:<user_w$} {:<display_w$} {:<id_w$}",
            r.num, r.rp, r.user, r.display, r.id,
        );
    }
}

/// Short one-line description of an entry, used in confirmation and result
/// output ("about to delete ...", "deleted ...").
fn describe(entry: &Entry) -> String {
    let user = &entry.credential.public_key_credential_user_entity;
    format!(
        "{} ({} / {})",
        decode_html_entities(&entry.rp_id),
        decode_html_entities(&user.name),
        decode_html_entities(&user.display_name)
    )
}

/// Parses a comma-separated list like "1,3,5" into zero-based, deduplicated,
/// range-checked indices into the `entries` list.
fn parse_indices(input: &str, len: usize) -> Result<Vec<usize>> {
    let mut result = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let n: usize = part
            .parse()
            .with_context(|| format!("'{}' is not a valid number", part))?;
        if n == 0 || n > len {
            bail!("{} is out of range (valid: 1-{})", n, len);
        }
        let idx = n - 1;
        if !result.contains(&idx) {
            result.push(idx);
        }
    }
    Ok(result)
}