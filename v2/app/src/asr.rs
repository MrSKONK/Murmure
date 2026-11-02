//! Reconnaissance vocale avec whisper.cpp.
use murmure_core::live::Recognizer;
use murmure_core::segment::SAMPLE_RATE;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

pub struct Model {
    _ctx: WhisperContext,
    state: WhisperState,
    threads: i32,
    /// mots douteux de la dernière transcription (voir `Recognizer::uncertain`)
    uncertain: Vec<String>,
    pub gpu: bool,
    /// carte graphique utilisée, ou « processeur » (affiché sur l'Accueil)
    pub device: String,
}

/// Carte graphique vue par whisper.cpp (Vulkan), s'il y en a une.
pub fn gpu_name() -> Option<String> {
    use whisper_rs_sys::*;
    unsafe {
        for i in 0..ggml_backend_dev_count() {
            let dev = ggml_backend_dev_get(i);
            if ggml_backend_dev_type(dev) == ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU {
                let desc = ggml_backend_dev_description(dev);
                if !desc.is_null() {
                    return Some(std::ffi::CStr::from_ptr(desc).to_string_lossy().trim().to_string());
                }
            }
        }
    }
    None
}

pub fn silence_logs() {
    // whisper.cpp écrit beaucoup sur la sortie d'erreur ; on le redirige vers le journal (désactivé)
    whisper_rs::install_logging_hooks();
}

impl Model {
    pub fn load(path: &std::path::Path, gpu: bool, threads: i32) -> Result<Model, String> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(gpu && cfg!(feature = "vulkan"));
        params.flash_attn(gpu && cfg!(feature = "vulkan"));
        let path = path.to_str().ok_or("chemin du modèle invalide")?;
        let ctx = WhisperContext::new_with_params(path, params).map_err(|e| e.to_string())?;
        let state = ctx.create_state().map_err(|e| e.to_string())?;
        let card = if gpu && cfg!(feature = "vulkan") { gpu_name() } else { None };
        let device = card.clone().unwrap_or_else(|| "processeur".into());
        Ok(Model { _ctx: ctx, state, threads, uncertain: Vec::new(), gpu: card.is_some(), device })
    }
}

impl Recognizer for Model {
    fn transcribe(
        &mut self,
        audio: &[f32],
        language: Option<&str>,
        prompt: Option<&str>,
        fast: bool,
    ) -> Result<(String, String), String> {
        // whisper.cpp ignore moins d'une seconde d'audio : on complète par du silence
        let mut padded;
        let mut audio = audio;
        if audio.len() < SAMPLE_RATE * 5 / 4 {
            padded = audio.to_vec();
            padded.resize(SAMPLE_RATE * 5 / 4, 0.0);
            audio = &padded;
        }
        let mut params = if fast {
            FullParams::new(SamplingStrategy::Greedy { best_of: 1 })
        } else {
            FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 })
        };
        params.set_n_threads(self.threads);
        params.set_language(Some(language.unwrap_or("auto")));
        params.set_translate(false);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true); // pas de « [Musique] », « ♪ » ni de bruits écrits en toutes lettres
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if let Some(p) = prompt {
            params.set_initial_prompt(p);
        }
        if fast {
            // passe rapide : un seul segment, sans repli en température. L'encodeur garde ses 30 s : le
            // raccourcir (audio_ctx) rendait le texte provisoire faux et le faisait se répéter en boucle.
            params.set_single_segment(true);
            params.set_no_timestamps(true);
            params.set_temperature_inc(0.0);
            // garde-fou contre les boucles : pas plus de jetons que la durée ne peut en contenir
            params.set_max_tokens((audio.len() * 8 / SAMPLE_RATE + 16) as i32);
        }
        self.state.full(params, audio).map_err(|e| e.to_string())?;
        let mut text = String::new();
        let mut doubts = Vec::new();
        for segment in self.state.as_iter() {
            if let Ok(s) = segment.to_str_lossy() {
                text.push_str(&s);
            }
            doubts.extend(doubtful_words(&segment));
        }
        self.uncertain = doubts;
        let detected = whisper_rs::get_lang_str(self.state.full_lang_id_from_state()).unwrap_or("").to_string();
        Ok((text.trim().to_string(), detected))
    }

    fn uncertain(&self) -> Vec<String> {
        self.uncertain.clone()
    }
}

/// En dessous, Whisper n'est pas sûr du mot : c'est là qu'il se trompe (noms propres, mots rares).
const DOUBT_BELOW: f32 = 0.5;

/// Mots d'un segment dont un jeton au moins a une probabilité faible. Un mot commence au jeton qui porte une espace.
fn doubtful_words(segment: &whisper_rs::WhisperSegment<'_>) -> Vec<String> {
    let mut out = Vec::new();
    let (mut word, mut lowest) = (String::new(), 1.0f32);
    let mut flush = |word: &mut String, lowest: &mut f32| {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if *lowest < DOUBT_BELOW && clean.chars().count() >= 3 {
            out.push(clean.to_string());
        }
        word.clear();
        *lowest = 1.0;
    };
    for i in 0..segment.n_tokens() {
        let Some(token) = segment.get_token(i) else { continue };
        let Ok(piece) = token.to_str_lossy() else { continue };
        if piece.starts_with("[_") || piece.starts_with("<|") {
            continue; // jetons spéciaux (début, fin, horodatage)
        }
        if piece.starts_with(' ') {
            flush(&mut word, &mut lowest);
        }
        word.push_str(&piece);
        lowest = lowest.min(token.token_probability());
    }
    flush(&mut word, &mut lowest);
    out
}
