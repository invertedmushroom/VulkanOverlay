// Processes input messages and handles hotkey events

use winapi::um::winuser::*;
use std::ptr::null_mut;
use std::mem::zeroed;
use crate::overlay::OverlayContent;
use crate::hotkey::WM_HOTKEY_ID;

pub fn process_input(overlay_content: &mut OverlayContent) -> bool {
    let mut msg: MSG = unsafe { zeroed() };

    unsafe {
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            println!("Received message: 0x{:X}", msg.message);
            if msg.message == WM_QUIT {
                return false;
            }

            match msg.message {
                WM_HOTKEY => {
                    println!("WM_HOTKEY received: wParam = {}", msg.wParam);
                    if msg.wParam as i32 == WM_HOTKEY_ID {
                        // Show the overlay
                        println!("Showing overlay");
                        overlay_content.visible = true;
                    }
                }
                _ => {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
    true
}