//! Raccourci clavier global en appuyer-pour-parler : crochet clavier bas niveau de Windows (appui ET relâchement,
//! touche seule bloquée comme dans la version 1). Mêmes noms de touches que la version 1 (« ² », « ctrl+space »).
use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Press,
    Release,
    Escape,
}

pub const MODIFIERS: [&str; 4] = ["ctrl", "alt", "shift", "windows"];

/// « Shift+Ctrl+D » -> « ctrl+shift+d » : modificateurs d'abord, dans l'ordre habituel.
pub fn normalize(parts: &[String]) -> String {
    let mut out: Vec<String> =
        MODIFIERS.iter().filter(|m| parts.iter().any(|p| p == *m)).map(|m| m.to_string()).collect();
    for p in parts {
        if !p.is_empty() && !MODIFIERS.contains(&p.as_str()) && !out.contains(p) {
            out.push(p.clone());
        }
    }
    out.join("+")
}

pub fn parse(spec: &str) -> Vec<String> {
    spec.split('+').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect()
}

struct Capture {
    pressed: Vec<String>,
    down: HashSet<String>,
    done: Sender<String>,
}

#[derive(Default)]
struct State {
    parts: Vec<String>,
    down: HashSet<String>,
    active: bool,
    suspended: bool,
    tx: Option<Sender<Event>>,
    capture: Option<Capture>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default))
}

pub fn set(spec: &str) {
    with(|s| {
        s.parts = parse(spec);
        s.down.clear();
        s.active = false;
    });
}

pub fn suspend(suspended: bool) {
    with(|s| {
        s.suspended = suspended;
        s.down.clear();
        s.active = false;
    });
}

/// Une touche du raccourci est-elle encore enfoncée ? (on attend avant de coller)
pub fn held() -> bool {
    with(|s| s.active || s.parts.iter().any(|p| s.down.contains(p)))
}

/// Traite une touche ; renvoie vrai si elle doit être bloquée.
fn handle(name: String, down: bool) -> bool {
    with(|s| {
        if let Some(c) = s.capture.as_mut() {
            if down {
                c.down.insert(name.clone());
                if !c.pressed.contains(&name) {
                    c.pressed.push(name);
                }
            } else {
                c.down.remove(&name);
                if c.down.is_empty() && !c.pressed.is_empty() {
                    let c = s.capture.take().unwrap();
                    let result = if c.pressed == ["esc"] { String::new() } else { normalize(&c.pressed) };
                    let _ = c.done.send(result);
                }
            }
            return false;
        }
        if s.suspended || s.parts.is_empty() {
            return false;
        }
        let send = |s: &State, e: Event| {
            if let Some(tx) = &s.tx {
                let _ = tx.send(e);
            }
        };
        if name == "esc" && down {
            send(s, Event::Escape);
        }
        if !s.parts.contains(&name) {
            return false;
        }
        if s.parts.len() == 1 {
            if down {
                if !s.active {
                    s.active = true; // la répétition automatique de la touche est ignorée
                    send(s, Event::Press);
                }
            } else {
                s.active = false;
                send(s, Event::Release);
            }
            return true; // touche seule : bloquée pour ne pas taper « ² » dans l'application
        }
        if down {
            s.down.insert(name);
            if !s.active && s.parts.iter().all(|p| s.down.contains(p)) {
                s.active = true;
                send(s, Event::Press);
            }
        } else {
            s.down.remove(&name);
            if s.active {
                s.active = false;
                send(s, Event::Release);
            }
        }
        false
    })
}

/// Écoute la prochaine touche ou combinaison pressée puis relâchée (« » si Échap ou délai dépassé).
pub fn capture(timeout: std::time::Duration) -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    with(|s| s.capture = Some(Capture { pressed: Vec::new(), down: HashSet::new(), done: tx }));
    let result = rx.recv_timeout(timeout).unwrap_or_default();
    with(|s| s.capture = None);
    result
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    pub fn key_name(vk: u32) -> String {
        let named = match VIRTUAL_KEY(vk as u16) {
            VK_CONTROL | VK_LCONTROL | VK_RCONTROL => "ctrl",
            VK_SHIFT | VK_LSHIFT | VK_RSHIFT => "shift",
            VK_MENU | VK_LMENU | VK_RMENU => "alt",
            VK_LWIN | VK_RWIN => "windows",
            VK_SPACE => "space",
            VK_RETURN => "enter",
            VK_ESCAPE => "esc",
            VK_TAB => "tab",
            VK_BACK => "backspace",
            VK_DELETE => "delete",
            VK_INSERT => "insert",
            VK_HOME => "home",
            VK_END => "end",
            VK_PRIOR => "page up",
            VK_NEXT => "page down",
            VK_LEFT => "left",
            VK_UP => "up",
            VK_RIGHT => "right",
            VK_DOWN => "down",
            VK_CAPITAL => "caps lock",
            VK_SNAPSHOT => "print screen",
            VK_PAUSE => "pause",
            VK_SCROLL => "scroll lock",
            VK_NUMLOCK => "num lock",
            VK_APPS => "menu",
            _ => "",
        };
        if !named.is_empty() {
            return named.to_string();
        }
        if (0x70..=0x87).contains(&vk) {
            return format!("f{}", vk - 0x6F);
        }
        // caractère de la touche dans la disposition du clavier (« ² », « a », « & »…)
        let ch = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR) } & 0x7FFF;
        match char::from_u32(ch).filter(|c| *c != '\0') {
            Some(c) => c.to_lowercase().collect(),
            None => format!("vk{vk}"),
        }
    }

    unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let injected = kb.flags.0 & LLKHF_INJECTED.0 != 0; // nos propres Ctrl+V
            if !injected {
                let msg = wparam.0 as u32;
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                if super::handle(key_name(kb.vkCode), down) {
                    return LRESULT(1);
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    pub fn run() {
        std::thread::spawn(|| unsafe {
            let module = GetModuleHandleW(None).ok();
            let installed = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), module.map(|m| m.into()), 0);
            if let Err(e) = installed {
                eprintln!("[Raccourci] crochet clavier impossible : {e}");
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }
}

/// Installe le crochet clavier ; les événements du raccourci arrivent sur `tx`.
pub fn start(tx: Sender<Event>) {
    with(|s| s.tx = Some(tx));
    #[cfg(windows)]
    win::run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_orders_modifiers() {
        let parts = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(normalize(&parts(&["d", "shift", "ctrl"])), "ctrl+shift+d");
        assert_eq!(normalize(&parts(&["²"])), "²");
        assert_eq!(parse(" Ctrl + Space "), ["ctrl", "space"]);
    }

    #[test]
    fn single_key_and_combo() {
        let (tx, rx) = std::sync::mpsc::channel();
        with(|s| s.tx = Some(tx));
        set("²");
        assert!(handle("²".into(), true));
        assert!(handle("²".into(), true)); // répétition automatique
        assert!(!handle("a".into(), true));
        assert!(handle("²".into(), false));
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [Event::Press, Event::Release]);

        set("ctrl+space");
        assert!(!handle("ctrl".into(), true));
        assert!(held());
        assert!(!handle("space".into(), true));
        assert!(!handle("space".into(), false));
        assert!(!handle("ctrl".into(), false));
        assert!(!held());
        handle("esc".into(), true);
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), [Event::Press, Event::Release, Event::Escape]);
        suspend(true);
        assert!(!handle("²".into(), true));
        suspend(false);
    }
}
