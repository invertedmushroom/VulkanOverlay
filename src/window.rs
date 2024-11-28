// Creates a transparent, click-through overlay window

use std::os::windows::ffi::OsStrExt;
use winapi::um::winuser::*;
use winapi::shared::windef::HWND;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::shared::minwindef::{UINT, WPARAM, LPARAM, LRESULT};
use std::ptr::null_mut;
use winapi::shared::minwindef::HINSTANCE;
pub fn create_overlay_window(title: &str, width: u32, height: u32) -> HWND {
    unsafe {
        let h_instance: HINSTANCE = GetModuleHandleW(null_mut());
        let class_name = to_wstring("OverlayWindowClass");

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: h_instance,
            lpszClassName: class_name.as_ptr(),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null_mut(),
        };

        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            to_wstring(title).as_ptr(),
            WS_POPUP,
            0,
            0,
            width as i32,
            height as i32,
            null_mut(),
            null_mut(),
            h_instance,
            null_mut(),
        );

        // Set window to be fully transparent
        SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        hwnd
    }
}

extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            unsafe { PostQuitMessage(0); }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) },
    }
}

// Helper function to convert &str to wide string
fn to_wstring(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
}
