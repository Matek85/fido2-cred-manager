//! Small terminal input helpers shared by every menu.

use anyhow::Result;
use std::io::{self, Write};

/// Reads one line of plain (non-hidden) input from the terminal. PINs are
/// always read separately via `rpassword`, which hides the input.
pub fn read_line(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line)
}

/// Reads a 1-based index from the user; returns `None` if they just press
/// Enter (used as "cancel" throughout the fingerprint menu).
pub fn pick_index(prompt: &str, len: usize) -> Result<Option<usize>> {
    loop {
        let input = read_line(prompt)?;
        let input = input.trim();
        if input.is_empty() {
            return Ok(None);
        }
        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= len => return Ok(Some(n - 1)),
            _ => println!("Please enter a number between 1 and {}.", len),
        }
    }
}