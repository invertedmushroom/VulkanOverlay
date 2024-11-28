// Manages global hotkeys for toggling overlay visibility

use winapi::um::winuser::*;
use winapi::shared::minwindef::UINT;
use std::ptr::null_mut;
use winapi::um::errhandlingapi::GetLastError;

pub const WM_HOTKEY_ID: i32 = 1;

pub fn register_hotkey() -> bool {
    let result = unsafe {
        RegisterHotKey(
            null_mut(),
            WM_HOTKEY_ID,
            MOD_ALT as UINT,
            0x52 as UINT,
        )
    };
    if result == 0 {
        let error = unsafe { GetLastError() };
        eprintln!("Failed to register hotkey. Error code: {}", error);
        false
    } else {
        true
    }
}

pub fn unregister_hotkey() {
    unsafe {
        UnregisterHotKey(null_mut(), WM_HOTKEY_ID);
    }
}
