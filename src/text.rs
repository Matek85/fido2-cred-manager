//! Pure text helpers for display: truncating long values and decoding HTML
//! character references. No device access, no I/O.

/// Truncates a string to at most `max_chars` characters, appending "…" if it
/// was cut short. Works on chars rather than bytes so it's safe for
/// non-ASCII display names (e.g. umlauts, emoji) and never panics on a
/// multi-byte UTF-8 boundary.
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Decodes HTML/XML character references (e.g. `&#246;`, `&ouml;`) back into
/// their actual character (e.g. `ö`) for display purposes.
///
/// Some identity providers (this shows up with certain Entra ID / ADFS
/// setups) store the WebAuthn `user.displayName` / `user.name` with such
/// entities already baked in, so the FIDO2 key itself ends up holding the
/// literal text `&#246;` instead of `ö`. This only affects how the name is
/// *printed*; the raw bytes used internally (e.g. for deleting a credential)
/// are never touched.
pub fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }

        // Try to read a bounded run of "entity-ish" characters up to a
        // closing ';'. `&#x10FFFF;` (10 chars before the ';') is the
        // longest legal numeric reference, so 12 is a safe ceiling.
        let mut candidate = String::new();
        let mut closed = false;
        for _ in 0..12 {
            match chars.peek() {
                Some(&';') => {
                    chars.next();
                    closed = true;
                    break;
                }
                Some(&next) if next.is_ascii_alphanumeric() || next == '#' => {
                    candidate.push(next);
                    chars.next();
                }
                _ => break,
            }
        }

        if closed {
            if let Some(decoded) = decode_entity(&candidate) {
                out.push(decoded);
                continue;
            }
        }

        // Not a recognised/closed entity: emit exactly what we consumed,
        // unchanged, so no data is ever silently dropped.
        out.push('&');
        out.push_str(&candidate);
        if closed {
            out.push(';');
        }
    }

    out
}

/// Decodes a single entity name/body (without the surrounding `&` and `;`)
/// into a character, if recognised. Covers numeric references (decimal and
/// hex, i.e. any Unicode code point) plus the named entities most likely to
/// show up in German/European names.
fn decode_entity(entity: &str) -> Option<char> {
    if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(dec) = entity.strip_prefix('#') {
        return dec.parse::<u32>().ok().and_then(char::from_u32);
    }
    Some(match entity {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{00A0}',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "Uuml" => 'Ü',
        "szlig" => 'ß',
        "eacute" => 'é',
        "egrave" => 'è',
        "agrave" => 'à',
        "ccedil" => 'ç',
        "ntilde" => 'ñ',
        _ => return None,
    })
}