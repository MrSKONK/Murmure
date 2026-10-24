//! Fonctions propres à Windows : collage, texte avant le curseur (UI Automation), lancement avec Windows.
//! Ailleurs, des équivalents neutres permettent de construire et tester l'application.
use std::time::Duration;

pub fn copy_text(text: &str) -> bool {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())).is_ok()
}

/// Colle `text` dans l'application active (presse-papiers puis Ctrl+V), et remet l'ancien presse-papiers.
pub fn paste(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    // None = lecture impossible (image…) : on ne touchera pas au presse-papiers après coup
    let previous = clipboard.get_text().ok();
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(50));
    send_ctrl_v();
    std::thread::sleep(Duration::from_millis(250)); // l'application lit le presse-papiers avant restauration
    if let Some(previous) = previous {
        let _ = clipboard.set_text(previous);
    }
    Ok(())
}

#[cfg(windows)]
fn send_ctrl_v() {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            },
        },
    };
    let inputs = [key(VK_CONTROL, false), key(VK_V, false), key(VK_V, true), key(VK_CONTROL, true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
fn send_ctrl_v() {}

/// Les quelques caractères juste avant le curseur dans l'application active, pour raccorder la dictée
/// (majuscule, espace). `None` si l'application ne l'expose pas ou tarde à répondre. Rien n'est conservé.
pub fn text_before_caret(max_chars: i32, timeout: Duration) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(caret::read(max_chars));
    });
    rx.recv_timeout(timeout).ok().flatten()
}

#[cfg(windows)]
mod caret {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Accessibility::*;

    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    fn com() -> Option<Com> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok().map(|_| Com) }
    }

    fn automation() -> windows::core::Result<IUIAutomation> {
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
    }

    pub fn read(max_chars: i32) -> Option<String> {
        let _com = com()?;
        let read = || -> windows::core::Result<Option<String>> {
            unsafe {
                let element = automation()?.GetFocusedElement()?;
                if element.CurrentIsPassword()?.as_bool() {
                    return Ok(None);
                }
                let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId)?;
                let selection = pattern.GetSelection()?;
                if selection.Length()? < 1 {
                    return Ok(None);
                }
                let caret = selection.GetElement(0)?;
                let before = caret.Clone()?;
                before.MoveEndpointByRange(TextPatternRangeEndpoint_End, &caret, TextPatternRangeEndpoint_Start)?;
                before.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -max_chars)?;
                Ok(Some(before.GetText(max_chars)?.to_string()))
            }
        };
        read().ok().flatten()
    }

    pub fn self_test() -> Result<(), String> {
        let _com = com().ok_or("COM indisponible")?;
        automation().map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod caret {
    pub fn read(_max_chars: i32) -> Option<String> {
        None
    }

    pub fn self_test() -> Result<(), String> {
        Ok(())
    }
}

pub use caret::self_test as caret_self_test;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "Murmure";
const OLD_NAMES: &[&str] = &["SpeechToText"]; // entrées laissées par les premières versions

#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(RUN_KEY).and_then(|k| k.get_value::<String, _>(RUN_NAME)).is_ok()
}

#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use winreg::{enums::*, RegKey};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
    for old in OLD_NAMES {
        let _ = key.delete_value(old);
    }
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value(RUN_NAME, &format!("\"{}\" --background", exe.display())).map_err(|e| e.to_string())
    } else {
        match key.delete_value(RUN_NAME) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(not(windows))]
pub fn autostart_enabled() -> bool {
    let _ = (RUN_KEY, RUN_NAME, OLD_NAMES);
    false
}

#[cfg(not(windows))]
pub fn set_autostart(_enabled: bool) -> Result<(), String> {
    Err("disponible seulement sous Windows".into())
}
