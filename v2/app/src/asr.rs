//! Reconnaissance vocale avec whisper.cpp.
use murmure_core::live::Recognizer;
use murmure_core::segment::SAMPLE_RATE;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

pub struct Model {
    _ctx: WhisperContext,
    state: WhisperState,
    threads: i32,
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
        Ok(Model { _ctx: ctx, state, threads, gpu: card.is_some(), device })
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
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if let Some(p) = prompt {
            params.set_initial_prompt(p);
        }
        if fast {
            // passe rapide : encodeur limité à la durée réelle de l'audio (1500 trames = 30 s)
            let ctx = (audio.len() * 50 / SAMPLE_RATE + 64).min(1500);
            params.set_audio_ctx(ctx as i32);
            params.set_single_segment(true);
            params.set_no_timestamps(true);
            params.set_temperature_inc(0.0);
        }
        self.state.full(params, audio).map_err(|e| e.to_string())?;
        let mut text = String::new();
        for segment in self.state.as_iter() {
            if let Ok(s) = segment.to_str_lossy() {
                text.push_str(&s);
            }
        }
        let detected = whisper_rs::get_lang_str(self.state.full_lang_id_from_state()).unwrap_or("").to_string();
        Ok((text.trim().to_string(), detected))
    }
}
