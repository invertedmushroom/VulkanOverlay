// Application entry point and main loop
mod window;
mod render;
mod input;
mod overlay;
mod hotkey;

use window::create_overlay_window;
use render::Renderer;
use input::process_input;
use overlay::OverlayContent;
use hotkey::{register_hotkey, unregister_hotkey};
use winapi::{shared::windef::HWND, um::{wingdi::RGB, winuser::{SetLayeredWindowAttributes, LWA_ALPHA, LWA_COLORKEY}}};
use winapi::shared::windef::POINT;
use winapi::um::winuser::{GetCursorPos, SetWindowPos, SWP_NOSIZE, SWP_NOZORDER, GetAsyncKeyState, VK_MENU};

fn main() {
    // Create the transparent, click-through window
    let hwnd: HWND = create_overlay_window("Radial Menu Overlay", 800, 600);

    // Register the global hotkey (Alt+R)
    if !register_hotkey() {
        eprintln!("Failed to register hotkey");
    }

    // Initialize Vulkan renderer
    let mut renderer = Renderer::new(hwnd).expect("Failed to initialize Vulkan renderer");

    // Initialize overlay content
    let mut overlay_content = OverlayContent::new();

    let mut prev_visibility = overlay_content.visible;

    let mut alt_pressed_prev = false;

    // Main application loop
    loop {
        // Process user input
        if !process_input(&mut overlay_content) {
            break;
        }

        // Check the state of the "Alt" key
        let alt_state = unsafe { GetAsyncKeyState(VK_MENU) };
        let alt_pressed = ((alt_state as u16) & 0x8000) != 0;

        // Detect changes in the "Alt" key state
        if alt_pressed != alt_pressed_prev {
            if !alt_pressed {
                // "Alt" key was released
                if overlay_content.visible {
                    // Hide the overlay
                    overlay_content.visible = false;
                }
            }
            alt_pressed_prev = alt_pressed;
        }

        // Check if visibility has changed
        if overlay_content.visible != prev_visibility {
            if overlay_content.visible {

                // Get mouse position
                let mut point: POINT = POINT { x: 0, y: 0 };
                unsafe {
                    GetCursorPos(&mut point);
                }

                // Adjust window position to center on mouse
                let window_width = 800; // Your window width
                let window_height = 600; // Your window height

                unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        point.x - window_width as i32 / 2,
                        point.y - window_height as i32 / 2,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER,
                    );
                }

                // Set window to fully opaque (alpha = magenta) //fix for OPAQUE not suporting transparency
                unsafe {
                    SetLayeredWindowAttributes(hwnd, RGB(255, 0, 255), 0, LWA_COLORKEY);
                }
            } else {
                // Overlay became hidden
                // Execute action if an item was selected
                if let Some(selected_segment) = overlay_content.selected_segment {
                    println!("Executing action for segment {}", selected_segment);
                    // TODO: Call the function or perform the action associated with the segment
                }

                // Reset the selected segment
                overlay_content.selected_segment = None;
                // Set window to fully transparent
                unsafe {
                    SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
                }
            }
            prev_visibility = overlay_content.visible;
        }

        // Render the overlay if visible
        if overlay_content.visible {
            renderer.render(&mut overlay_content, hwnd).expect("Rendering failed");
        }

        // Sleep to reduce CPU usage
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    // Clean up resources
    unregister_hotkey();
    renderer.cleanup();
}
