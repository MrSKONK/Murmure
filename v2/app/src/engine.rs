//! Moteur de dictée : raccourci -> micro -> whisper.cpp au fil de l'eau -> dictionnaire -> correction -> collage.
//!
//! Pendant l'enregistrement, un fil transcrit chaque morceau terminé par une pause et propose un texte
//! provisoire à la pastille ; au relâchement, il ne reste que la fin à transcrire.
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use murmure_core::cleanup::{self, Options};
use murmure_core::dictionary;
use murmure_core::history::History;
use murmure_core::live::Recognizer;
use murmure_core::live::{Live, Step};
use murmure_core::segment::SAMPLE_RATE;
use murmure_core::settings::Settings;
use serde_json::{json, Value};

use crate::audio::Recorder;
use crate::{hotkey, models, system};

#[cfg(feature = "whisper")]
use crate::asr::Model;

#[cfg(not(feature = "whisper"))]
pub struct Model {
    device: String,
}

#[cfg(not(feature = "whisper"))]
impl Model {
    fn load(_: &std::path::Path, _: bool, _: i32) -> Result<Model, String> {
        Err("cette version a été construite sans whisper.cpp".into())
    }
}

#[cfg(not(feature = "whisper"))]
impl Recognizer for Model {
    fn transcribe(&mut self, _: &[f32], _: Option<&str>, _: Option<&str>, _: bool) -> Result<(String, String), String> {
        Err("moteur absent".into())
    }
}

/// Retours vers l'interface (fenêtre, icône, pastille).
pub trait Ui: Send + Sync {
    fn state_changed(&self);
    fn notice(&self, text: &str, error: bool);
    fn preview(&self, text: &str);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Loading,
    Idle,
    Recording,
    Processing,
    Error,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Loading => "loading",
            State::Idle => "idle",
            State::Recording => "recording",
            State::Processing => "processing",
            State::Error => "error",
        }
    }
}

struct Inner {
    state: State,
    message: String,
    last_text: String,
    dictations: u64,
    job: u64,
    cancelled_job: u64,
    last_use: Instant,
}

pub struct Engine {
    settings: Mutex<Settings>,
    inner: Mutex<Inner>,
    recorder: Mutex<Recorder>,
    model: Mutex<Option<Model>>,
    load_lock: Mutex<()>,
    history: Arc<History>,
    ui: Arc<dyn Ui>,
}

fn model_keys(s: &Settings) -> (&str, bool, u64) {
    (&s.whisper_model, s.gpu, s.cpu_threads)
}

/// Fils de calcul : réglage, ou tous les cœurs (8 au plus, au-delà le gain est nul).
pub fn threads(setting: u64) -> i32 {
    let all = thread::available_parallelism().map_or(4, |n| n.get());
    let n = if setting == 0 { all.min(8) } else { setting as usize };
    n.clamp(1, 64) as i32
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Engine {
    pub fn new(settings: Settings, history: Arc<History>, ui: Arc<dyn Ui>) -> Arc<Engine> {
        Arc::new(Engine {
            settings: Mutex::new(settings),
            inner: Mutex::new(Inner {
                state: State::Loading,
                message: String::new(),
                last_text: String::new(),
                dictations: 0,
                job: 0,
                cancelled_job: 0,
                last_use: Instant::now(),
            }),
            recorder: Mutex::new(Recorder::default()),
            model: Mutex::new(None),
            load_lock: Mutex::new(()),
            history,
            ui,
        })
    }

    /// Lance le chargement du modèle, l'écoute du raccourci et la mise en veille du modèle.
    pub fn start(self: &Arc<Self>, keys: Receiver<hotkey::Event>) {
        hotkey::set(&self.settings().hotkey);
        let me = self.clone();
        thread::spawn(move || me.ensure_model(false));
        let me = self.clone();
        thread::spawn(move || {
            for event in keys {
                match event {
                    hotkey::Event::Press => me.press(),
                    hotkey::Event::Release => me.release(),
                    hotkey::Event::Escape => {
                        if matches!(me.state(), State::Recording | State::Processing) {
                            me.cancel();
                        }
                    }
                }
            }
        });
        let me = self.clone();
        thread::spawn(move || me.idle_watch());
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn state(&self) -> State {
        lock(&self.inner).state
    }

    pub fn message(&self) -> String {
        lock(&self.inner).message.clone()
    }

    pub fn last_text(&self) -> String {
        lock(&self.inner).last_text.clone()
    }

    pub fn level(&self) -> f32 {
        lock(&self.recorder).shared.level()
    }

    pub fn status(&self) -> Value {
        let device = lock(&self.model).as_ref().map(|m| m.device.clone());
        let i = lock(&self.inner);
        json!({"state": i.state.name(), "message": i.message, "model_loaded": device.is_some(),
               "device": device, "last_text": i.last_text, "dictations": i.dictations})
    }

    fn set(&self, state: State, message: &str) {
        {
            let mut i = lock(&self.inner);
            i.state = state;
            i.message = message.to_string();
        }
        self.ui.state_changed();
    }

    fn set_message(&self, message: &str) {
        lock(&self.inner).message = message.to_string();
        self.ui.state_changed();
    }

    // ------------------------------------------------------------------ //
    //  Modèle                                                              //
    // ------------------------------------------------------------------ //

    fn try_load(&self, path: &std::path::Path, s: &Settings) -> Result<Model, String> {
        let threads = threads(s.cpu_threads);
        match Model::load(path, s.gpu, threads) {
            Err(e) if s.gpu => {
                eprintln!("[Moteur] carte graphique inutilisable ({e}), calcul sur le processeur");
                Model::load(path, false, threads)
            }
            other => other,
        }
    }

    /// Charge le modèle (le télécharge s'il manque, le retélécharge s'il est abîmé). `quiet` : sans changer
    /// l'état affiché (chargement à la demande pendant une dictée).
    fn ensure_model(&self, quiet: bool) -> bool {
        let _guard = lock(&self.load_lock);
        if lock(&self.model).is_some() {
            if !quiet && matches!(self.state(), State::Loading | State::Error) {
                self.set(State::Idle, "");
            }
            return true;
        }
        let show = |msg: &str| if quiet { self.set_message(msg) } else { self.set(State::Loading, msg) };
        show("Chargement du modèle…");
        let s = self.settings();
        let download = |s: &Settings| {
            models::ensure(&s.whisper_model, |p| show(&format!("Téléchargement du modèle… {} %", (p * 100.0) as u32)))
        };
        let result = download(&s).and_then(|path| {
            show("Chargement du modèle…");
            self.try_load(&path, &s).or_else(|e| {
                eprintln!("[Moteur] modèle illisible ({e}) : nouveau téléchargement");
                show("Modèle endommagé : nouveau téléchargement…");
                self.ui.notice("Modèle endommagé : nouveau téléchargement…", false);
                let _ = std::fs::remove_file(&path);
                download(&s).and_then(|path| self.try_load(&path, &s))
            })
        });
        match result {
            Ok(mut model) => {
                // le tout premier calcul est lent (la carte graphique prépare ses programmes) : fait d'avance,
                // pour que le texte en direct apparaisse dès la première dictée
                let silence = vec![0.0f32; SAMPLE_RATE];
                let language = (s.whisper_language != "auto").then_some(s.whisper_language.as_str());
                let _ = model.transcribe(&silence, language, None, true);
                let _ = model.transcribe(&silence, language, None, false);
                *lock(&self.model) = Some(model);
                lock(&self.inner).last_use = Instant::now();
                if quiet {
                    self.set_message("");
                } else {
                    self.set(State::Idle, "");
                }
                true
            }
            Err(e) => {
                let msg = format!("Chargement du modèle impossible : {e}");
                eprintln!("[Moteur] {msg}");
                self.set(State::Error, &msg);
                self.ui.notice(&msg, true);
                false
            }
        }
    }

    fn idle_watch(self: Arc<Self>) {
        loop {
            thread::sleep(Duration::from_secs(20));
            let minutes = self.settings().unload_after_min;
            let idle = {
                let i = lock(&self.inner);
                matches!(i.state, State::Idle | State::Error)
                    && i.last_use.elapsed() > Duration::from_secs(minutes * 60)
            };
            if minutes > 0 && idle && lock(&self.model).take().is_some() {
                self.ui.state_changed();
            }
        }
    }

    // ------------------------------------------------------------------ //
    //  Réglages                                                            //
    // ------------------------------------------------------------------ //

    pub fn apply_settings(self: &Arc<Self>, new: Settings) {
        let old = std::mem::replace(&mut *lock(&self.settings), new.clone());
        if old.hotkey != new.hotkey {
            hotkey::set(&new.hotkey);
        }
        if model_keys(&old) != model_keys(&new) {
            *lock(&self.model) = None;
            let me = self.clone();
            thread::spawn(move || me.ensure_model(false));
        }
        self.ui.state_changed();
    }

    // ------------------------------------------------------------------ //
    //  Raccourci                                                           //
    // ------------------------------------------------------------------ //

    fn press(self: &Arc<Self>) {
        let s = self.settings();
        let state = self.state();
        if state == State::Recording && s.hotkey_mode == "toggle" {
            self.finish_recording();
            return;
        }
        if state == State::Loading {
            self.ui.notice("Le modèle est en cours de chargement…", false);
            return;
        }
        if !matches!(state, State::Idle | State::Error) {
            return;
        }
        if let Err(e) = lock(&self.recorder).start(&s.mic) {
            let msg = format!("Microphone indisponible : {e}");
            eprintln!("[Moteur] {msg}");
            self.set(State::Idle, "");
            self.ui.notice(&msg, true);
            return;
        }
        let job = {
            let mut i = lock(&self.inner);
            i.job += 1;
            i.job
        };
        self.set(State::Recording, "");
        if lock(&self.model).is_none() {
            let me = self.clone(); // modèle mis en veille : rechargé pendant qu'on parle
            thread::spawn(move || me.ensure_model(true));
        }
        let me = self.clone();
        thread::spawn(move || me.work(job, s));
    }

    fn release(&self) {
        if self.state() == State::Recording && self.settings().hotkey_mode == "hold" {
            self.finish_recording();
        }
    }

    fn finish_recording(&self) {
        lock(&self.recorder).stop();
        self.set(State::Processing, "");
    }

    /// Croix de la pastille ou Échap : abandonne la dictée en cours sans rien coller.
    pub fn cancel(&self) {
        let state = self.state();
        {
            let mut i = lock(&self.inner);
            if !matches!(state, State::Recording | State::Processing) || i.cancelled_job == i.job {
                return;
            }
            i.cancelled_job = i.job;
        }
        if state == State::Recording {
            lock(&self.recorder).stop();
            self.set(State::Idle, "");
        }
        self.ui.notice("Dictée annulée.", false);
    }

    fn cancelled(&self, job: u64) -> bool {
        lock(&self.inner).cancelled_job == job
    }

    // ------------------------------------------------------------------ //
    //  Dictée                                                              //
    // ------------------------------------------------------------------ //

    fn work(self: Arc<Self>, job: u64, s: Settings) {
        if let Err(e) = self.dictate(job, &s) {
            eprintln!("[Moteur] {e}");
            self.ui.notice(&e, true);
        }
        let current = {
            let mut i = lock(&self.inner);
            i.last_use = Instant::now();
            i.job == job && i.state == State::Processing // une autre dictée a pu commencer entre-temps
        };
        if current {
            self.set(State::Idle, "");
        }
    }

    fn dictate(&self, job: u64, s: &Settings) -> Result<(), String> {
        let shared = lock(&self.recorder).shared.clone();
        let language = (s.whisper_language != "auto").then(|| s.whisper_language.clone());
        let mut live = Live::new(language.clone(), dictionary::initial_prompt(&s.dictionary), s.live_preview);
        let mut audio: Vec<f32> = Vec::new();
        let copy_new = |audio: &mut Vec<f32>| {
            let samples = lock(&shared.samples);
            if samples.len() > audio.len() {
                audio.extend_from_slice(&samples[audio.len()..]);
            }
        };

        // pendant l'enregistrement : morceaux terminés transcrits au fil de l'eau
        while shared.recording.load(Ordering::Relaxed) {
            copy_new(&mut audio);
            let mut step = Step::Idle;
            if let Ok(mut model) = self.model.try_lock() {
                if let Some(model) = model.as_mut() {
                    step = live.step(model, &audio)?;
                }
            }
            if self.cancelled(job) {
                return Ok(());
            }
            match step {
                Step::Preview(text) | Step::Committed(text) if s.live_preview => self.ui.preview(&text),
                Step::Idle => thread::sleep(Duration::from_millis(120)),
                _ => {}
            }
        }
        copy_new(&mut audio);
        if self.cancelled(job) {
            return Ok(());
        }
        if audio.len() < SAMPLE_RATE / 2 {
            self.ui.notice("Enregistrement trop court, ignoré.", false);
            return Ok(());
        }

        // fin : seul le dernier morceau reste à transcrire
        if lock(&self.model).is_none() && !self.ensure_model(true) {
            return Ok(());
        }
        let (text, detected) = {
            let mut model = lock(&self.model);
            let model = model.as_mut().ok_or("modèle déchargé")?;
            live.finish(model, &audio)?
        };
        if self.cancelled(job) {
            return Ok(());
        }
        if s.log_transcripts {
            eprintln!("[Whisper] {text:?}");
        }
        let lang = language.unwrap_or(detected);
        let text = postprocess(&text, &lang, s);
        if text.is_empty() {
            self.ui.notice("Aucune parole détectée.", false);
            return Ok(());
        }
        if s.log_transcripts {
            eprintln!("[Final]   {text:?}");
        }
        {
            let mut i = lock(&self.inner);
            i.last_text = text.trim().to_string();
            i.dictations += 1;
        }
        if s.history_enabled {
            let duration = audio.len() as f64 / SAMPLE_RATE as f64;
            if let Err(e) = self.history.add(text.trim(), duration, &lang) {
                eprintln!("[Moteur] Historique non enregistré : {e}");
            }
        }
        // Ctrl+V ne doit pas partir pendant que des touches du raccourci sont encore enfoncées
        let until = Instant::now() + Duration::from_secs(1);
        while hotkey::held() && Instant::now() < until {
            thread::sleep(Duration::from_millis(20));
        }
        system::paste(&text).map_err(|e| format!("Collage impossible : {e}"))
    }
}

/// Dictionnaire, correction par règles, puis raccord avec le texte déjà présent avant le curseur.
fn postprocess(text: &str, lang: &str, s: &Settings) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let text = dictionary::apply(text, &s.dictionary);
    let options = Options {
        fillers: s.remove_fillers,
        commands: s.spoken_punctuation,
        french: lang == "fr",
        nbsp: s.french_nbsp,
        addresses: s.spoken_addresses,
        short_no_period: s.short_no_period,
    };
    let text = cleanup::clean(&text, options);
    if text.is_empty() || !s.fit_to_context {
        return text;
    }
    let before = system::text_before_caret(12, Duration::from_millis(400));
    cleanup::fit_to_context(&text, before.as_deref(), &dictionary::keep_case(&s.dictionary))
}
