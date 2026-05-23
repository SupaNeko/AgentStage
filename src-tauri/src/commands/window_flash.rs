#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{FlashWindowEx, FLASHWINFO, FLASHW_STOP, FLASHW_TRAY};

/// Flash the taskbar button a specified number of times.
/// Uses Win32 FlashWindowEx for precise control.
#[tauri::command]
pub fn flash_taskbar(window: tauri::Window, count: u32) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = window.hwnd() {
            let flash_info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd,
                dwFlags: FLASHW_TRAY,
                uCount: count,
                dwTimeout: 0,
            };
            unsafe {
                let _ = FlashWindowEx(&flash_info);
            }
        }
    }
}

/// Stop flashing the taskbar button.
#[tauri::command]
pub fn clear_flash(window: tauri::Window) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = window.hwnd() {
            let flash_info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd,
                dwFlags: FLASHW_STOP,
                uCount: 0,
                dwTimeout: 0,
            };
            unsafe {
                let _ = FlashWindowEx(&flash_info);
            }
        }
    }
}
