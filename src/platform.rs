//! Platform-specific startup/shutdown behavior.
//!
//! Everything in here exists because of Windows: raw HID access to FIDO2
//! keys needs administrator rights, and a double-clicked console program
//! closes its window the moment it exits. On other systems the functions
//! are no-ops. This is also the only file containing `unsafe` code (Win32
//! calls), which is why it is kept separate from everything else.

/// Result of the startup elevation check.
// `Relaunched` and `Failed` are only ever produced by the Windows
// implementation, so on other systems the compiler would warn about them.
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Elevation {
    /// Already elevated (or not on Windows): just carry on.
    Ok,
    /// A new, elevated copy of this program was started; this instance
    /// must exit without doing anything else.
    Relaunched,
    /// Elevation was needed but could not be obtained (e.g. the UAC prompt
    /// was declined).
    Failed,
}

/// Makes sure the program runs with administrator rights on Windows.
///
/// The embedded manifest (see build.rs) normally makes Windows show the UAC
/// prompt before the program even starts. This function is a second,
/// independent safety net: if we still end up running non-elevated (for
/// example because the manifest is missing from an old build), we ask
/// Windows to start a fresh elevated copy of ourselves ("runas" verb, which
/// shows the UAC prompt) and let this instance exit.
///
/// Without administrator rights Windows hides FIDO2 keys from raw HID
/// access, so continuing non-elevated would only produce a confusing
/// "no key found" error.
#[cfg(windows)]
pub fn ensure_elevated() -> Elevation {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    #[link(name = "advapi32")]
    extern "system" {
        fn OpenProcessToken(process: *mut c_void, access: u32, token: *mut *mut c_void) -> i32;
        fn GetTokenInformation(
            token: *mut c_void,
            class: i32,
            info: *mut c_void,
            length: u32,
            return_length: *mut u32,
        ) -> i32;
    }
    #[link(name = "shell32")]
    extern "system" {
        // Returns a value > 32 on success (legacy HINSTANCE convention).
        fn ShellExecuteW(
            window: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }

    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION_CLASS: i32 = 20; // TokenElevation
    const SW_SHOWNORMAL: i32 = 1;

    // Step 1: ask Windows whether our process token is elevated.
    // SAFETY: all pointers passed below point to valid, live local
    // variables of exactly the size the Win32 functions expect, and the
    // token handle is closed again right after use.
    let is_elevated: Option<bool> = unsafe {
        let mut token: *mut c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            None
        } else {
            let mut elevation: u32 = 0; // TOKEN_ELEVATION { TokenIsElevated: u32 }
            let mut returned: u32 = 0;
            let ok = GetTokenInformation(
                token,
                TOKEN_ELEVATION_CLASS,
                &mut elevation as *mut u32 as *mut c_void,
                std::mem::size_of::<u32>() as u32,
                &mut returned,
            );
            CloseHandle(token);
            if ok == 0 {
                None
            } else {
                Some(elevation != 0)
            }
        }
    };

    // Already elevated, or we couldn't tell: carry on. (If the check itself
    // fails we deliberately do NOT relaunch, to rule out any relaunch loop.)
    if is_elevated != Some(false) {
        return Elevation::Ok;
    }

    // Step 2: relaunch ourselves elevated. The "runas" verb triggers the
    // UAC prompt.
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: could not determine the path of this program: {}", e);
            return Elevation::Failed;
        }
    };
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();

    println!("Administrator rights are required - requesting elevation (UAC prompt)...");

    // SAFETY: `verb` and `file` are valid, null-terminated UTF-16 strings
    // that outlive the call; the remaining pointer arguments are null,
    // which ShellExecuteW allows.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    if result > 32 {
        Elevation::Relaunched
    } else {
        eprintln!(
            "Error: administrator rights are required to access the security key, \
             but they could not be obtained (was the UAC prompt cancelled?)."
        );
        Elevation::Failed
    }
}

/// Only Windows restricts raw HID access to FIDO devices, so nothing to do
/// on other systems.
#[cfg(not(windows))]
pub fn ensure_elevated() -> Elevation {
    Elevation::Ok
}

/// Waits for Enter before the program exits, but only when this process
/// owns its console window exclusively - i.e. it was started by double-
/// click (or by the UAC elevation from the embedded manifest) rather than
/// from an already open terminal. In a normal terminal we don't want to
/// force an extra keypress.
#[cfg(windows)]
pub fn pause_if_own_console() {
    use std::io::{self, Write};

    #[link(name = "kernel32")]
    extern "system" {
        // Returns how many processes are attached to the current console
        // (0 on failure). If our buffer is too small it returns the needed
        // count, which is fine: we only care whether the count is exactly 1.
        fn GetConsoleProcessList(process_list: *mut u32, process_count: u32) -> u32;
    }

    let mut list = [0u32; 2];
    // SAFETY: `list` is a valid, writable buffer of `list.len()` u32 values,
    // which is exactly what the Win32 function expects.
    let attached = unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) };

    if attached == 1 {
        print!("\nPress Enter to exit...");
        io::stdout().flush().ok();
        let mut line = String::new();
        io::stdin().read_line(&mut line).ok();
    }
}

/// On non-Windows systems there is no "console closes instantly" problem.
#[cfg(not(windows))]
pub fn pause_if_own_console() {}