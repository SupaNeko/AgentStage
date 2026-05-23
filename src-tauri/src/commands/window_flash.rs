use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{FlashWindowEx, FLASHWINFO, FLASHW_STOP, FLASHW_TRAY};

static FLASH_CALLED: AtomicBool = AtomicBool::new(false);

/// Flash the taskbar button a specified number of times.
/// Uses Win32 FlashWindowEx for precise control.
#[tauri::command]
pub fn flash_taskbar(window: tauri::Window, count: u32) {
    #[cfg(target_os = "windows")]
    {
        match window.hwnd() {
            Ok(hwnd) => {
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
                FLASH_CALLED.store(true, Ordering::Relaxed);
                crate::logger::debug(&format!("[flash_taskbar] hwnd={:?} count={} ok", hwnd, count));
            }
            Err(e) => {
                crate::logger::error(&format!("[flash_taskbar] failed to get hwnd: {}", e));
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        crate::logger::warn("[flash_taskbar] not supported on this platform");
    }
}

/// Stop flashing the taskbar button.
#[tauri::command]
pub fn clear_flash(window: tauri::Window) {
    #[cfg(target_os = "windows")]
    {
        match window.hwnd() {
            Ok(hwnd) => {
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
                crate::logger::debug(&format!("[clear_flash] hwnd={:?} ok", hwnd));
            }
            Err(e) => {
                crate::logger::error(&format!("[clear_flash] failed to get hwnd: {}", e));
            }
        }
    }
}
