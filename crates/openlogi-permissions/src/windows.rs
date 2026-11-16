//! Windows process privilege status.

#![expect(
    unsafe_code,
    reason = "raw win32: OpenProcessToken + GetTokenInformation — localized here"
)]

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Whether this process runs with an elevated (administrator) token.
///
/// A token that cannot be read reports `false`: the caller only labels the
/// process, it never grants anything on this answer.
#[must_use]
pub fn is_elevated() -> bool {
    // SAFETY: the pseudo-handle from `GetCurrentProcess` needs no closing; the
    // token handle is closed below; `elevation` is a plain struct sized for
    // `TokenElevation`.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            (&raw mut elevation).cast(),
            u32::try_from(std::mem::size_of::<TOKEN_ELEVATION>()).unwrap_or(0),
            &raw mut returned,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// The SID of the account this process runs as, in its string form
/// (`S-1-5-21-…`). An elevated process and an unelevated one of the same
/// account share it, so it names "this user" across the UAC boundary.
#[must_use]
pub fn current_user_sid() -> Option<String> {
    // `TOKEN_USER` is followed in the same buffer by the SID it points at; the
    // u64 backing keeps the struct aligned and is larger than any SID.
    let mut buffer = [0u64; 16];
    // SAFETY: the token handle is closed below; `buffer` is writable for the
    // size passed and aligned for `TOKEN_USER`; the string from
    // `ConvertSidToStringSidW` is NUL-terminated and freed with `LocalFree`.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) == 0 {
            return None;
        }
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            u32::try_from(std::mem::size_of_val(&buffer)).unwrap_or(0),
            &raw mut returned,
        );
        CloseHandle(token);
        if ok == 0 {
            return None;
        }
        let user = buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW((*user).User.Sid, &raw mut text) == 0 || text.is_null() {
            return None;
        }
        // A SID string is at most 184 characters plus its terminator.
        let len = (0..256).take_while(|&i| *text.add(i) != 0).count();
        let sid = String::from_utf16(std::slice::from_raw_parts(text, len)).ok();
        LocalFree(text.cast());
        sid
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_current_user_has_an_account_sid() {
        let sid = super::current_user_sid().expect("the process token names its user");
        assert!(sid.starts_with("S-1-"), "{sid}");
    }
}
