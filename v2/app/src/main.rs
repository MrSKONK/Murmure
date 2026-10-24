//! Murmure 2 : application Tauri. La fenêtre reprend l'interface de la version 1 (`ui/index.html`) ; ses appels
//! `window.pywebview.api.*` sont redirigés vers les commandes ci-dessous.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "whisper")]
mod asr;
mod audio;
mod checks;
mod engine;
mod grammar;
#[cfg_attr(not(windows), allow(dead_code))]
mod hotkey;
mod models;
mod paths;
mod system;
mod updater;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use murmure_core::history::History;
use murmure_core::release::Release;
use murmure_core::settings::Settings;
use serde_json::{json, Value};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use engine::{Engine, State, Ui};

/// Adaptateur : l'interface de la version 1 appelle `window.pywebview.api.nom(args)`.
const PYWEBVIEW_SHIM: &str = r#"
(() => {
  const ARGS = { save_settings: ["new"], copy_history: ["entry_id"], delete_history: ["entry_id"] };
  const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
  window.pywebview = { api: new Proxy({}, { get: (_, name) => (...a) =>
    invoke(name, Object.fromEntries((ARGS[name] || []).map((k, i) => [k, a[i]]))) }) };
  window.addEventListener("DOMContentLoaded", () => window.dispatchEvent(new Event("pywebviewready")));
})();
"#;

const PILL_HEIGHT: f64 = 56.0;

pub struct App {
    handle: AppHandle,
    engine: OnceLock<Arc<Engine>>,
    grammar: OnceLock<Arc<grammar::Grammar>>,
    history: Arc<History>,
    update: Mutex<Value>,
    release: Mutex<Option<Release>>,
    pill_gen: AtomicU64,
    pill_size: Mutex<(f64, f64)>,
    quitting: AtomicBool,
}

fn app(handle: &AppHandle) -> Arc<App> {
    handle.state::<Arc<App>>().inner().clone()
}

impl App {
    fn engine(&self) -> &Arc<Engine> {
        self.engine.get().expect("moteur démarré")
    }

    fn main_window(&self) -> Option<WebviewWindow> {
        self.handle.get_webview_window("main")
    }

    fn eval(&self, callback: &str, payload: &Value) {
        if let Some(w) = self.main_window() {
            let _ = w.eval(format!("window.{callback} && window.{callback}({payload})"));
        }
    }

    fn show_main(&self) {
        if let Some(w) = self.main_window() {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }

    fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        if let Some(engine) = self.engine.get() {
            engine.cancel();
        }
        self.handle.exit(0);
    }

    fn settings_json(&self) -> Value {
        let mut s = serde_json::to_value(self.engine().settings()).unwrap_or_default();
        s["autostart"] = Value::Bool(system::autostart_enabled());
        s
    }

    fn save_settings(&self, patch: &Value) -> Value {
        let engine = self.engine();
        let old = engine.settings();
        let mut new = old.merged(patch);
        if new.autostart != old.autostart {
            if let Err(e) = system::set_autostart(new.autostart) {
                self.notice(&format!("Démarrage automatique impossible : {e}"), true);
                new.autostart = old.autostart;
            }
        }
        if let Err(e) = new.save(&paths::settings_path()) {
            self.notice(&format!("Réglages non enregistrés : {e}"), true);
        }
        self.history.set_limit(new.history_limit as usize);
        if !new.show_overlay {
            self.pill("hidden", "");
        }
        if new.grammar_enabled {
            if let Some(g) = self.grammar.get() {
                g.install(); // télécharge ou charge le correcteur s'il ne l'est pas encore
            }
        }
        engine.apply_settings(new);
        self.settings_json()
    }

    // ---------------------------------------------------------------- //
    //  Pastille                                                         //
    // ---------------------------------------------------------------- //

    fn pill_window(&self) -> Option<WebviewWindow> {
        self.handle.get_webview_window("pill")
    }

    /// Place la pastille en bas, au centre de l'écran principal.
    fn place_pill(&self, window: &WebviewWindow) {
        let (width, height) = *self.pill_size.lock().unwrap();
        let Ok(Some(monitor)) = window.primary_monitor() else { return };
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let (w, h) = ((width * scale).round(), (height * scale).round());
        let x = area.position.x as f64 + (area.size.width as f64 - w) / 2.0;
        let y = area.position.y as f64 + area.size.height as f64 - h - 28.0 * scale;
        let _ = window.set_size(tauri::PhysicalSize::new(w as u32, h as u32));
        let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
    }

    /// state : recording | processing | notice | error | hidden
    fn pill(&self, state: &str, text: &str) {
        let Some(window) = self.pill_window() else { return };
        let generation = self.pill_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let enabled = self.engine.get().is_none_or(|e| e.settings().show_overlay);
        let _ = window.emit_to("pill", "pill", json!({"state": state, "text": text}));
        if state == "hidden" || !enabled {
            // l'animation de sortie se joue avant de masquer la fenêtre
            let me = app(&self.handle);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(220));
                if me.pill_gen.load(Ordering::SeqCst) == generation {
                    if let Some(w) = me.pill_window() {
                        let _ = w.hide();
                    }
                }
            });
            return;
        }
        self.place_pill(&window);
        show_without_focus(&window);
        if matches!(state, "notice" | "error") {
            let me = app(&self.handle);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(if state_is_error(&me) { 4000 } else { 2600 }));
                if me.pill_gen.load(Ordering::SeqCst) == generation {
                    me.refresh_pill();
                }
            });
        }
    }

    /// Pastille selon l'état du moteur (après un avis, ou à chaque changement d'état).
    fn refresh_pill(&self) {
        let engine = self.engine();
        match engine.state() {
            State::Recording => self.pill("recording", ""),
            State::Processing => {
                let message = engine.message();
                self.pill("processing", if message.is_empty() { "Transcription…" } else { &message })
            }
            _ => self.pill("hidden", ""),
        }
    }

    fn notice(&self, text: &str, error: bool) {
        self.pill(if error { "error" } else { "notice" }, text);
    }

    fn refresh_tray(&self, state: State) {
        let Some(tray) = self.handle.tray_by_id("main") else { return };
        let (icon, tip): (&[u8], &str) = match state {
            State::Loading => (include_bytes!("../icons/tray-loading.png"), "Chargement du modèle…"),
            State::Idle => (include_bytes!("../icons/tray-idle.png"), "Prêt"),
            State::Recording => (include_bytes!("../icons/tray-recording.png"), "Enregistrement…"),
            State::Processing => (include_bytes!("../icons/tray-processing.png"), "Transcription…"),
            State::Error => (include_bytes!("../icons/tray-error.png"), "Erreur — ouvrez la fenêtre pour les détails"),
        };
        if let Ok(image) = Image::from_bytes(icon) {
            let _ = tray.set_icon(Some(image));
        }
        let _ = tray.set_tooltip(Some(format!("Murmure — {tip}")));
    }

    // ---------------------------------------------------------------- //
    //  Mises à jour                                                     //
    // ---------------------------------------------------------------- //

    fn update_json(&self) -> Value {
        let mut u = self.update.lock().unwrap().clone();
        u["current"] = json!(updater::VERSION);
        u["supported"] = json!(cfg!(windows) && !cfg!(debug_assertions));
        u
    }

    fn set_update(&self, changes: Value) {
        {
            let mut u = self.update.lock().unwrap();
            if let (Value::Object(u), Value::Object(changes)) = (&mut *u, changes) {
                u.extend(changes);
            }
        }
        self.eval("__update", &self.update_json());
    }

    fn update_state(&self) -> String {
        self.update.lock().unwrap()["state"].as_str().unwrap_or("").to_string()
    }

    fn check_update(&self) {
        if !self.update_json()["supported"].as_bool().unwrap_or(false) {
            self.set_update(json!({"state": "unsupported"}));
            return;
        }
        if matches!(self.update_state().as_str(), "checking" | "downloading" | "installing") {
            return;
        }
        self.set_update(json!({"state": "checking", "error": ""}));
        match updater::check() {
            Err(e) => self.set_update(json!({"state": "error", "error": format!("Vérification impossible : {e}")})),
            Ok(None) => {
                *self.release.lock().unwrap() = None;
                self.set_update(json!({"state": "uptodate", "version": "", "notes": ""}));
            }
            Ok(Some(release)) => {
                let notes: String = release.notes.chars().take(2000).collect();
                self.set_update(json!({"state": "available", "version": release.version, "notes": notes}));
                *self.release.lock().unwrap() = Some(release);
            }
        }
    }

    fn install_update(&self) {
        let Some(release) = self.release.lock().unwrap().clone() else { return };
        if matches!(self.update_state().as_str(), "downloading" | "installing") {
            return;
        }
        if matches!(self.engine().state(), State::Recording | State::Processing) {
            self.set_update(json!({"state": "available", "error": "Terminez la dictée en cours avant d'installer."}));
            return;
        }
        self.set_update(json!({"state": "downloading", "progress": 0.0, "error": ""}));
        let mut shown = 0.0f32;
        let result = updater::download(&release, |p| {
            if p - shown >= 0.01 || p >= 1.0 {
                shown = p;
                self.set_update(json!({"progress": (p * 1000.0).round() / 1000.0}));
            }
        })
        .and_then(|path| {
            self.set_update(json!({"state": "installing"}));
            updater::launch_installer(&path)
        });
        match result {
            Ok(()) => self.quit(), // l'installateur remplace les fichiers puis relance Murmure
            Err(e) => self.set_update(json!({"state": "available", "error": format!("Mise à jour impossible : {e}")})),
        }
    }
}

fn state_is_error(app: &App) -> bool {
    app.engine.get().is_some_and(|e| e.state() == State::Error)
}

/// Affiche la pastille sans prendre le focus : le curseur reste dans l'application de saisie.
fn show_without_focus(window: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let hwnd = HWND(hwnd.0);
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted = style | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
            if wanted != style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            }
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        return;
    }
    let _ = window.show();
}

struct AppUi(AppHandle);

impl Ui for AppUi {
    fn state_changed(&self) {
        let app = app(&self.0);
        let Some(engine) = app.engine.get() else { return };
        app.refresh_tray(engine.state());
        app.refresh_pill();
        app.eval("__status", &engine.status());
    }

    fn notice(&self, text: &str, error: bool) {
        let app = app(&self.0);
        app.notice(text, error);
        if let Some(engine) = app.engine.get() {
            app.eval("__status", &engine.status());
        }
    }

    fn preview(&self, text: &str) {
        let app = app(&self.0);
        if let Some(w) = app.pill_window() {
            let _ = w.emit_to("pill", "preview", text);
        }
    }
}

// -------------------------------------------------------------------- //
//  Commandes appelées par l'interface                                   //
// -------------------------------------------------------------------- //

#[tauri::command(rename_all = "snake_case")]
fn get_settings(app: tauri::State<'_, Arc<App>>) -> Value {
    app.settings_json()
}

#[tauri::command(rename_all = "snake_case", async)]
fn save_settings(app: tauri::State<'_, Arc<App>>, new: Value) -> Value {
    app.save_settings(&new)
}

#[tauri::command(rename_all = "snake_case")]
fn get_status(app: tauri::State<'_, Arc<App>>) -> Value {
    app.engine().status()
}

#[tauri::command(rename_all = "snake_case", async)]
fn list_microphones() -> Vec<String> {
    audio::list_microphones()
}

#[tauri::command(rename_all = "snake_case", async)]
fn capture_hotkey() -> String {
    hotkey::suspend(true);
    let result = hotkey::capture(Duration::from_secs(10));
    hotkey::suspend(false);
    result
}

#[tauri::command(rename_all = "snake_case")]
fn copy_last(app: tauri::State<'_, Arc<App>>) -> bool {
    let text = app.engine().last_text();
    !text.is_empty() && system::copy_text(&text)
}

#[tauri::command(rename_all = "snake_case")]
fn get_system() -> Value {
    let cpus = std::thread::available_parallelism().map_or(0, |n| n.get());
    json!({"cpus": cpus, "history_path": paths::history_path(), "version": updater::VERSION,
           "engine": "whisper.cpp", "backend": backend(), "models_path": paths::models_dir()})
}

fn backend() -> &'static str {
    if cfg!(feature = "vulkan") {
        "Vulkan"
    } else {
        "CPU"
    }
}

#[tauri::command(rename_all = "snake_case")]
fn get_history(app: tauri::State<'_, Arc<App>>) -> Value {
    serde_json::to_value(app.history.entries()).unwrap_or_default()
}

#[tauri::command(rename_all = "snake_case")]
fn copy_history(app: tauri::State<'_, Arc<App>>, entry_id: String) -> bool {
    app.history.get(&entry_id).is_some_and(|e| system::copy_text(&e.text))
}

#[tauri::command(rename_all = "snake_case")]
fn delete_history(app: tauri::State<'_, Arc<App>>, entry_id: String) -> bool {
    app.history.delete(&entry_id)
}

#[tauri::command(rename_all = "snake_case")]
fn clear_history(app: tauri::State<'_, Arc<App>>) -> bool {
    app.history.clear();
    true
}

#[tauri::command(rename_all = "snake_case")]
fn get_grammar(app: tauri::State<'_, Arc<App>>) -> Value {
    app.grammar.get().map(|g| g.status()).unwrap_or_default()
}

/// Appelé par la fenêtre du correcteur une fois Grammalecte chargé.
#[tauri::command(rename_all = "snake_case")]
fn grammar_ready(app: tauri::State<'_, Arc<App>>, error: String) {
    if let Some(g) = app.grammar.get() {
        g.page_ready(error);
    }
}

/// Appelé par la fenêtre du correcteur avec les erreurs relevées.
#[tauri::command(rename_all = "snake_case")]
fn grammar_result(app: tauri::State<'_, Arc<App>>, id: u64, result: String) {
    if let Some(g) = app.grammar.get() {
        g.page_result(id, result);
    }
}

#[tauri::command(rename_all = "snake_case")]
fn get_update(app: tauri::State<'_, Arc<App>>) -> Value {
    app.update_json()
}

#[tauri::command(rename_all = "snake_case", async)]
fn check_update(app: tauri::State<'_, Arc<App>>) {
    app.check_update();
}

#[tauri::command(rename_all = "snake_case", async)]
fn install_update(app: tauri::State<'_, Arc<App>>) {
    app.install_update();
}

#[tauri::command(rename_all = "snake_case")]
fn quit_app(app: tauri::State<'_, Arc<App>>) {
    app.quit();
}

#[tauri::command(rename_all = "snake_case")]
fn cancel_dictation(app: tauri::State<'_, Arc<App>>) {
    app.engine().cancel();
}

#[tauri::command(rename_all = "snake_case")]
fn pill_level(app: tauri::State<'_, Arc<App>>) -> f32 {
    app.engine().level()
}

/// Taille de la pastille, mesurée par la page (la bulle de texte en direct l'agrandit vers le haut).
#[tauri::command(rename_all = "snake_case")]
fn pill_resize(app: tauri::State<'_, Arc<App>>, width: f64, height: f64) {
    *app.pill_size.lock().unwrap() = (width.clamp(120.0, 900.0), height.clamp(PILL_HEIGHT, 400.0));
    if let Some(w) = app.pill_window() {
        if w.is_visible().unwrap_or(false) {
            app.place_pill(&w);
        }
    }
}

// -------------------------------------------------------------------- //

fn setup(handle: &AppHandle, background: bool, check_grammar: Option<String>) -> tauri::Result<()> {
    let settings = Settings::load(&paths::settings_path());
    let history = Arc::new(History::new(paths::history_path(), settings.history_limit as usize));
    let state = Arc::new(App {
        handle: handle.clone(),
        engine: OnceLock::new(),
        grammar: OnceLock::new(),
        history: history.clone(),
        update: Mutex::new(json!({"state": "idle", "version": "", "notes": "", "progress": 0.0, "error": ""})),
        release: Mutex::new(None),
        pill_gen: AtomicU64::new(0),
        pill_size: Mutex::new((300.0, PILL_HEIGHT)),
        quitting: AtomicBool::new(false),
    });
    handle.manage(state.clone());
    let push = handle.clone();
    let grammar = grammar::Grammar::new(handle.clone(), move |status| app(&push).eval("__grammar", status));
    let _ = state.grammar.set(grammar.clone());
    if let Some(report) = check_grammar {
        // autotest : ni fenêtre, ni icône, ni moteur de dictée
        checks::grammar_check(handle.clone(), grammar, report);
        return Ok(());
    }

    WebviewWindowBuilder::new(handle, "main", WebviewUrl::App("index.html".into()))
        .title("Murmure")
        .inner_size(1080.0, 720.0)
        .min_inner_size(820.0, 560.0)
        .background_color(Color(246, 246, 248, 255))
        .initialization_script(PYWEBVIEW_SHIM)
        .visible(!background)
        .build()?;
    WebviewWindowBuilder::new(handle, "pill", WebviewUrl::App("pill.html".into()))
        .title("Murmure — pastille")
        .inner_size(300.0, PILL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;

    let open = MenuItem::with_id(handle, "open", "Ouvrir", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(handle, &[&open, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray-loading.png"))?)
        .tooltip("Murmure — Chargement du modèle…")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|handle, event| match event.id.as_ref() {
            "open" => app(handle).show_main(),
            "quit" => app(handle).quit(),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                app(tray.app_handle()).show_main();
            }
        })
        .build(handle)?;

    #[cfg(feature = "whisper")]
    asr::silence_logs();
    if settings.grammar_enabled {
        grammar.install();
    }
    let engine = Engine::new(settings, history, Arc::new(AppUi(handle.clone())), grammar);
    let _ = state.engine.set(engine.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    hotkey::start(tx);
    engine.start(rx);

    // mises à jour : au démarrage puis toutes les 12 h
    let me = state.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        while !me.quitting.load(Ordering::SeqCst) {
            if me.engine().settings().auto_update && me.update_json()["supported"].as_bool().unwrap_or(false) {
                me.check_update();
            }
            std::thread::sleep(Duration::from_secs(12 * 3600));
        }
    });
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = checks::run(&args) {
        std::process::exit(code);
    }
    // --check-grammar rapport.txt : autotest du correcteur, qui a besoin de l'application (fenêtre cachée)
    let check_grammar =
        args.iter().position(|a| a == "--check-grammar").map(|i| args.get(i + 1).cloned().unwrap_or_default());
    let background = check_grammar.is_some() || args.iter().any(|a| a == "--background");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|handle, _args, _cwd| app(handle).show_main()))
        .register_uri_scheme_protocol(grammar::SCHEME, |_ctx, request| grammar::serve(&request))
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_status,
            list_microphones,
            capture_hotkey,
            copy_last,
            get_system,
            get_history,
            copy_history,
            delete_history,
            clear_history,
            get_grammar,
            grammar_ready,
            grammar_result,
            get_update,
            check_update,
            install_update,
            quit_app,
            cancel_dictation,
            pill_level,
            pill_resize,
        ])
        .setup(move |tauri_app| {
            setup(tauri_app.handle(), background, check_grammar.clone())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // fermer la fenêtre la masque : Murmure reste dans la zone de notification
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && !app(window.app_handle()).quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("démarrage de Murmure impossible")
        .run(|_, _| {});
}
