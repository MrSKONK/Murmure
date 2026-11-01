//! Autotests de l'exécutable, lancés par la construction automatique (GitHub Actions) :
//! `murmure --check rapport.txt` et `murmure --check-transcribe audio.wav rapport.txt [modèle]`.
use std::fmt::Write as _;
use std::time::Instant;

use murmure_core::cleanup::{self, Options};
use murmure_core::live::{Live, Step};
use murmure_core::resample::Resampler;
use murmure_core::segment::SAMPLE_RATE;

use crate::{engine, models, paths, system, updater};

pub fn run(args: &[String]) -> Option<i32> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let (code, report) = match arg(1)? {
        "--check" => check(),
        "--check-transcribe" => transcribe(arg(2).unwrap_or("test.wav"), arg(4).unwrap_or("tiny")),
        _ => return None,
    };
    match arg(if arg(1) == Some("--check") { 2 } else { 3 }) {
        Some(path) => {
            let _ = std::fs::write(path, &report);
        }
        None => print!("{report}"),
    }
    Some(code)
}

fn check() -> (i32, String) {
    let mut out = String::new();
    let mut ok = true;
    let _ = writeln!(out, "version: {}", updater::VERSION);
    let _ = writeln!(out, "moteur: whisper.cpp ({})", if cfg!(feature = "vulkan") { "Vulkan" } else { "CPU" });
    let o = Options { french: true, ..Options::default() };
    let cleaned = cleanup::clean("euh bonjour virgule ça va point d'interrogation", o);
    let expected = "Bonjour, ça va\u{202f}?";
    ok &= cleaned == expected;
    let _ = writeln!(out, "correction: {} ({cleaned:?})", if cleaned == expected { "ok" } else { "ÉCHEC" });
    match system::caret_self_test() {
        Ok(()) => {
            let _ = writeln!(out, "ui automation: ok");
        }
        Err(e) => {
            ok = false;
            let _ = writeln!(out, "ui automation: ÉCHEC ({e})");
        }
    }
    let _ = writeln!(out, "micros: {}", crate::audio::list_microphones().len());
    let _ = writeln!(out, "réglages: {}", paths::settings_path().display());
    let _ = writeln!(out, "modèles: {}", paths::models_dir().display());
    let _ = writeln!(out, "{}", if ok { "OK" } else { "ÉCHEC" });
    (i32::from(!ok), out)
}

fn read_wav(path: &str) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / max).collect()
        }
        hound::SampleFormat::Float => reader.into_samples::<f32>().filter_map(Result::ok).collect(),
    };
    let mut out = Vec::new();
    Resampler::new(spec.sample_rate, spec.channels, SAMPLE_RATE as u32).push(&samples, &mut out);
    Ok(out)
}

#[cfg(feature = "whisper")]
fn transcribe(wav: &str, model: &str) -> (i32, String) {
    let mut out = String::new();
    let result = (|| -> Result<String, String> {
        let audio = read_wav(wav)?;
        let _ = writeln!(out, "audio: {:.1} s", audio.len() as f32 / SAMPLE_RATE as f32);
        let t = Instant::now();
        let path = models::ensure(model, |_| {})?;
        let _ = writeln!(out, "téléchargement: {:.1} s ({})", t.elapsed().as_secs_f32(), path.display());
        let t = Instant::now();
        let gpu = cfg!(feature = "vulkan");
        let mut recognizer = crate::asr::Model::load(&path, gpu, engine::threads(0))
            .or_else(|_| crate::asr::Model::load(&path, false, engine::threads(0)))?;
        let _ = writeln!(out, "chargement: {:.1} s (carte graphique : {})", t.elapsed().as_secs_f32(), recognizer.gpu);
        // comme pendant une dictée : l'audio arrive par paquets de 200 ms
        let language = std::env::var("MURMURE_CHECK_LANG").ok().filter(|l| l != "auto");
        let mut live = Live::new(language, None, true);
        let (mut committed, mut previews) = (0, 0);
        let t = Instant::now();
        let mut end = 0;
        while end < audio.len() {
            end = (end + SAMPLE_RATE / 5).min(audio.len());
            match live.step(&mut recognizer, &audio[..end])? {
                Step::Committed(_) => committed += 1,
                Step::Preview(_) => previews += 1,
                Step::Idle => {}
            }
        }
        let live_secs = t.elapsed().as_secs_f32();
        let t = Instant::now();
        let (text, lang) = live.finish(&mut recognizer, &audio)?;
        let _ = writeln!(out, "temps réel: {committed} morceaux, {previews} aperçus en {live_secs:.1} s");
        let _ = writeln!(out, "fin de transcription: {:.2} s après le relâchement", t.elapsed().as_secs_f32());
        let _ = writeln!(out, "langue: {lang}");
        Ok(text)
    })();
    match result {
        Ok(text) if !text.trim().is_empty() => {
            let _ = writeln!(out, "texte: {text}\nOK");
            (0, out)
        }
        Ok(_) => {
            let _ = writeln!(out, "texte vide\nÉCHEC");
            (1, out)
        }
        Err(e) => {
            let _ = writeln!(out, "erreur: {e}\nÉCHEC");
            (1, out)
        }
    }
}

#[cfg(not(feature = "whisper"))]
fn transcribe(_wav: &str, _model: &str) -> (i32, String) {
    let _ = (read_wav, models::path, engine::threads, Instant::now, Live::new, Step::Idle);
    (1, "construit sans whisper.cpp\nÉCHEC\n".into())
}

/// `--check-grammar rapport.txt` : télécharge Grammalecte, le charge dans sa fenêtre cachée et corrige une phrase
/// témoin (mêmes attentes que l'autotest de la version 1). L'application se ferme ensuite.
pub fn grammar_check(handle: tauri::AppHandle, grammar: std::sync::Arc<crate::grammar::Grammar>, report: String) {
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut out = String::new();
        let code = match grammar.install_blocking() {
            Err(e) => {
                let _ = writeln!(out, "erreur: {e}\nÉCHEC");
                1
            }
            Ok(()) => {
                let _ = writeln!(out, "prêt en {:.1} s", start.elapsed().as_secs_f32());
                let sentence = "Ils mange des pommes et il va a Paris.";
                let expected = "Ils mangent des pommes et il va à Paris.";
                let t = Instant::now();
                let got = grammar.correct(sentence);
                let _ = writeln!(out, "correction en {} ms", t.elapsed().as_millis());
                let _ = writeln!(out, "{sentence:?} -> {got:?}");
                let ok = got == expected;
                let _ = writeln!(out, "{}", if ok { "OK" } else { "ÉCHEC" });
                i32::from(!ok)
            }
        };
        let _ = std::fs::write(&report, &out);
        handle.exit(code);
    });
}
