//! Découpage de la parole pendant l'enregistrement, pour transcrire au fil de l'eau : chaque pause marque la fin
//! d'un morceau, transcrit aussitôt pendant qu'on continue de parler. Détection par l'énergie du signal, avec un
//! seuil adapté au bruit de fond du micro.

pub const SAMPLE_RATE: usize = 16_000;
/// Trame d'analyse : 30 ms.
pub const FRAME: usize = SAMPLE_RATE * 30 / 1000;

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// pause qui termine un morceau
    pub min_silence_ms: usize,
    /// morceau minimal : en dessous, on attend la pause suivante (Whisper aime le contexte)
    pub min_segment_ms: usize,
    /// au-delà, coupure forcée au moment le plus calme des dernières secondes
    pub max_segment_ms: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config { min_silence_ms: 600, min_segment_ms: 2000, max_segment_ms: 24_000 }
    }
}

const fn frames(ms: usize) -> usize {
    ms * SAMPLE_RATE / 1000 / FRAME
}

/// Volume (RMS) de chaque trame complète.
pub fn frame_rms(audio: &[f32]) -> Vec<f32> {
    audio.chunks_exact(FRAME).map(|f| (f.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt()).collect()
}

/// Seuil parole / silence : trois fois le bruit de fond (20ᵉ centile des trames), borné.
pub fn speech_threshold(audio: &[f32]) -> f32 {
    let mut rms = frame_rms(audio);
    if rms.is_empty() {
        return 0.01;
    }
    rms.sort_by(f32::total_cmp);
    let floor = rms[rms.len() / 5];
    (floor * 3.0).clamp(0.006, 0.04)
}

/// Assez de parole pour mériter une transcription (au moins 240 ms au-dessus du seuil).
pub fn has_speech(audio: &[f32], threshold: f32) -> bool {
    frame_rms(audio).iter().filter(|&&r| r > threshold).count() >= frames(240)
}

/// Fin du morceau qui commence à `start`, si une pause (ou la longueur maximale) le termine déjà.
pub fn next_cut(audio: &[f32], start: usize, threshold: f32, cfg: &Config) -> Option<usize> {
    let rms = frame_rms(audio.get(start..)?);
    let (min_silence, min_segment, max_segment) =
        (frames(cfg.min_silence_ms).max(1), frames(cfg.min_segment_ms), frames(cfg.max_segment_ms).max(2));
    let mut speech = 0;
    let mut silence = 0;
    for (i, &r) in rms.iter().enumerate() {
        if r > threshold {
            speech += 1;
            silence = 0;
        } else {
            silence += 1;
        }
        let at = |frame: usize| start + frame * FRAME;
        if silence >= min_silence {
            if speech >= frames(240) && i + 1 >= min_segment {
                return Some(at(i + 1 - min_silence / 2)); // au milieu de la pause
            }
            if speech < frames(240) && silence >= frames(3000) {
                return Some(at(i + 1 - frames(500))); // long silence sans parole : on avance
            }
        }
        if i + 1 >= max_segment {
            // pas de pause : coupure au moment le plus calme des 3 dernières secondes
            let from = (i + 1).saturating_sub(frames(3000));
            let quiet = (from..=i).min_by(|&a, &b| rms[a].total_cmp(&rms[b])).unwrap_or(i);
            return Some(at(quiet.max(1)));
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Signal de test : (durée en ms, parole ou silence).
    pub fn signal(parts: &[(usize, bool)]) -> Vec<f32> {
        let mut out = Vec::new();
        for &(ms, voiced) in parts {
            for i in 0..ms * SAMPLE_RATE / 1000 {
                let noise = ((i * 7919 % 1000) as f32 / 1000.0 - 0.5) * 0.004;
                let voice = if voiced { (i as f32 * 0.07).sin() * 0.15 } else { 0.0 };
                out.push(voice + noise);
            }
        }
        out
    }

    const CFG: Config = Config { min_silence_ms: 600, min_segment_ms: 2000, max_segment_ms: 24_000 };

    #[test]
    fn threshold_follows_noise_floor() {
        let quiet = signal(&[(2000, false), (2000, true)]);
        let t = speech_threshold(&quiet);
        assert!(t > 0.005 && t < 0.02, "{t}");
        assert!(has_speech(&signal(&[(1000, true)]), t));
        assert!(!has_speech(&signal(&[(3000, false)]), t));
        assert!(!has_speech(&signal(&[(3000, false), (100, true)]), t));
    }

    #[test]
    fn cuts_in_the_middle_of_a_pause() {
        let a = signal(&[(300, false), (2500, true), (900, false), (1500, true)]);
        let t = speech_threshold(&a);
        let cut = next_cut(&a, 0, t, &CFG).expect("pause détectée");
        let ms = cut * 1000 / SAMPLE_RATE;
        assert!((2800..=3400).contains(&ms), "{ms}");
        // le morceau suivant (parole en cours, sans pause finale) n'est pas encore terminé
        assert_eq!(next_cut(&a, cut, t, &CFG), None);
    }

    #[test]
    fn short_phrase_waits_for_more_context() {
        let a = signal(&[(800, true), (700, false), (500, true)]);
        assert_eq!(next_cut(&a, 0, speech_threshold(&a), &CFG), None);
    }

    #[test]
    fn short_breath_is_not_a_pause() {
        let a = signal(&[(2500, true), (300, false), (2500, true)]);
        assert_eq!(next_cut(&a, 0, speech_threshold(&a), &CFG), None);
    }

    #[test]
    fn forced_cut_without_pause() {
        let a = signal(&[(30_000, true)]);
        let cut = next_cut(&a, 0, 0.01, &CFG).expect("coupure forcée");
        assert!(cut <= 24 * SAMPLE_RATE && cut >= 20 * SAMPLE_RATE, "{cut}");
    }

    #[test]
    fn long_silence_is_skipped() {
        let a = signal(&[(4000, false)]);
        let cut = next_cut(&a, 0, 0.01, &CFG).expect("silence sauté");
        assert!(!has_speech(&a[..cut], 0.01));
    }
}
