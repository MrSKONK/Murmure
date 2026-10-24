//! Transcription en temps réel : les morceaux terminés par une pause sont transcrits pendant qu'on parle, un texte
//! provisoire est proposé pour la pastille, et au relâchement il ne reste que la fin à transcrire.
use crate::segment::{self, Config, SAMPLE_RATE};

/// Moteur de reconnaissance (whisper.cpp dans l'application, simulé dans les tests).
pub trait Recognizer {
    /// Texte et langue détectée. `fast` : passe rapide pour le texte provisoire.
    fn transcribe(
        &mut self,
        audio: &[f32],
        language: Option<&str>,
        prompt: Option<&str>,
        fast: bool,
    ) -> Result<(String, String), String>;
}

#[derive(Debug, PartialEq)]
pub enum Step {
    /// rien à faire pour l'instant
    Idle,
    /// un morceau terminé a été transcrit ; texte définitif jusqu'ici
    Committed(String),
    /// texte provisoire (définitif + fin en cours)
    Preview(String),
}

pub struct Live {
    committed_until: usize,
    parts: Vec<String>,
    language: Option<String>,
    detected: String,
    prompt: Option<String>,
    preview: bool,
    previewed_at: usize,
    /// aperçu figé jusqu'ici (texte de `frozen`), pour ne recalculer que les dernières secondes
    preview_until: usize,
    frozen: Vec<String>,
    cfg: Config,
}

/// Découpage pendant la dictée. Whisper est bien plus juste avec du contexte : un morceau n'est transcrit
/// définitivement qu'après 10 s de parole et une vraie pause (ou à 28 s, sa limite étant 30 s). Une dictée
/// courte est donc transcrite d'un seul tenant au relâchement ; une longue n'a plus que sa fin à transcrire.
pub const LIVE: Config = Config { min_silence_ms: 700, min_segment_ms: 10_000, max_segment_ms: 28_000 };

/// Aperçu : à chaque petite pause après 3 s (ou à 8 s), le texte affiché est figé et seules les secondes
/// suivantes sont recalculées. Chaque aperçu reste court et rapide, même pendant une longue dictée.
const PREVIEW_FREEZE: Config = Config { min_silence_ms: 350, min_segment_ms: 3000, max_segment_ms: 8000 };

/// Phrases que Whisper invente sur du silence ou du bruit (génériques de sous-titres).
const HALLUCINATIONS: &[&str] = &[
    "sous-titres réalisés par la communauté d'amara.org",
    "sous-titres réalisés para la communauté d'amara.org",
    "sous-titrage st' 501",
    "sous-titrage société radio-canada",
    "merci d'avoir regardé cette vidéo",
    "merci d'avoir regardé",
    "thank you for watching",
    "thanks for watching",
    "abonnez-vous",
    "...",
];

fn is_hallucination(text: &str) -> bool {
    let t = text.trim().trim_end_matches(['.', '!', ' ']).replace('’', "'").to_lowercase();
    t.is_empty() || HALLUCINATIONS.iter().any(|h| t == h.trim_end_matches('.'))
}

/// Fin du texte déjà transcrit, donnée à Whisper comme contexte du morceau suivant.
fn tail(text: &str, max_chars: usize) -> &str {
    let start = text.char_indices().rev().nth(max_chars).map_or(0, |(i, _)| i);
    let tail = &text[start..];
    if start == 0 {
        tail
    } else {
        tail.split_once(' ').map_or(tail, |(_, rest)| rest)
    }
}

impl Live {
    /// `language` : `None` pour la détection automatique (fixée ensuite par le premier morceau).
    pub fn new(language: Option<String>, prompt: Option<String>, preview: bool) -> Self {
        Self::with_config(language, prompt, preview, LIVE)
    }

    pub fn with_config(language: Option<String>, prompt: Option<String>, preview: bool, cfg: Config) -> Self {
        Live {
            committed_until: 0,
            parts: Vec::new(),
            language,
            detected: String::new(),
            prompt,
            preview,
            previewed_at: 0,
            preview_until: 0,
            frozen: Vec::new(),
            cfg,
        }
    }

    pub fn text(&self) -> String {
        self.parts.join(" ")
    }

    /// Texte définitif suivi de l'aperçu figé.
    fn shown(&self) -> String {
        self.parts.iter().chain(&self.frozen).map(String::as_str).collect::<Vec<_>>().join(" ")
    }

    fn context(&self, fast: bool) -> Option<String> {
        let text = if fast { self.shown() } else { self.text() };
        let tail = tail(&text, 200);
        match (&self.prompt, tail.is_empty()) {
            (Some(p), true) => Some(p.clone()),
            (Some(p), false) => Some(format!("{p} {tail}")),
            (None, false) => Some(tail.to_string()),
            (None, true) => None,
        }
    }

    fn run(&mut self, r: &mut impl Recognizer, audio: &[f32], fast: bool) -> Result<String, String> {
        let (text, detected) = r.transcribe(audio, self.language.as_deref(), self.context(fast).as_deref(), fast)?;
        if !fast && self.language.is_none() && !detected.is_empty() {
            self.language = Some(detected.clone()); // la langue du premier morceau vaut pour la suite
        }
        if self.detected.is_empty() && !fast {
            self.detected = detected;
        }
        let text = text.trim().to_string();
        Ok(if is_hallucination(&text) { String::new() } else { text })
    }

    /// Une étape pendant l'enregistrement (au plus une transcription). `audio` : tout l'enregistrement à 16 kHz.
    pub fn step(&mut self, r: &mut impl Recognizer, audio: &[f32]) -> Result<Step, String> {
        let threshold = segment::speech_threshold(audio);
        if let Some(cut) = segment::next_cut(audio, self.committed_until, threshold, &self.cfg) {
            let piece = &audio[self.committed_until..cut];
            self.committed_until = cut;
            self.preview_until = cut; // le texte définitif remplace l'aperçu de ce morceau
            self.frozen.clear();
            if segment::has_speech(piece, threshold) {
                let text = self.run(r, piece, false)?;
                if !text.is_empty() {
                    self.parts.push(text);
                }
            }
            return Ok(Step::Committed(self.text()));
        }
        if !self.preview {
            return Ok(Step::Idle);
        }
        let start = self.preview_until.max(self.committed_until);
        if let Some(cut) = segment::next_cut(audio, start, threshold, &PREVIEW_FREEZE) {
            let piece = &audio[start..cut];
            self.preview_until = cut;
            if segment::has_speech(piece, threshold) {
                let text = self.run(r, piece, true)?;
                if !text.is_empty() {
                    self.frozen.push(text);
                }
                return Ok(Step::Preview(self.shown()));
            }
            return Ok(Step::Idle);
        }
        let pending = &audio[start..];
        if pending.len() >= SAMPLE_RATE * 6 / 10
            && audio.len() >= self.previewed_at + SAMPLE_RATE * 4 / 10
            && segment::has_speech(pending, threshold)
        {
            self.previewed_at = audio.len();
            let partial = self.run(r, pending, true)?;
            let mut text = self.shown();
            if !partial.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&partial);
            }
            return Ok(Step::Preview(text));
        }
        Ok(Step::Idle)
    }

    /// Fin de l'enregistrement : transcrit ce qui reste. Renvoie le texte complet et la langue.
    pub fn finish(mut self, r: &mut impl Recognizer, audio: &[f32]) -> Result<(String, String), String> {
        let threshold = segment::speech_threshold(audio);
        let rest = audio.get(self.committed_until..).unwrap_or(&[]);
        // un enregistrement très court n'a pas de bruit de fond mesurable : on transcrit quand même
        if segment::has_speech(rest, threshold) || (self.parts.is_empty() && audio.len() < 3 * SAMPLE_RATE) {
            let text = self.run(r, rest, false)?;
            if !text.is_empty() {
                self.parts.push(text);
            }
        }
        let language =
            if self.detected.is_empty() { self.language.clone().unwrap_or_default() } else { self.detected.clone() };
        Ok((self.text(), language))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::tests::signal;

    /// Reconnaît « mot<durée en dixièmes de seconde> » et note les appels.
    #[derive(Default)]
    struct Fake {
        calls: Vec<(usize, bool, Option<String>)>,
        reply: Option<String>,
    }

    impl Recognizer for Fake {
        fn transcribe(
            &mut self,
            audio: &[f32],
            language: Option<&str>,
            prompt: Option<&str>,
            fast: bool,
        ) -> Result<(String, String), String> {
            self.calls.push((audio.len(), fast, prompt.map(str::to_string)));
            let text = self.reply.clone().unwrap_or_else(|| format!("mot{}", audio.len() * 10 / SAMPLE_RATE));
            Ok((text, language.unwrap_or("fr").to_string()))
        }
    }

    fn feed(live: &mut Live, fake: &mut Fake, audio: &[f32]) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut end = 0;
        while end < audio.len() {
            end = (end + SAMPLE_RATE / 5).min(audio.len()); // toutes les 200 ms
            let step = live.step(fake, &audio[..end]).unwrap();
            if step != Step::Idle {
                steps.push(step);
            }
        }
        steps
    }

    #[test]
    fn segments_committed_while_speaking() {
        let audio = signal(&[(300, false), (2500, true), (900, false), (2500, true), (900, false), (1200, true)]);
        let mut fake = Fake::default();
        let short = Config { min_silence_ms: 600, min_segment_ms: 2000, max_segment_ms: 24_000 };
        let mut live = Live::with_config(Some("fr".into()), Some("Vocabulaire : Murmure.".into()), false, short);
        let steps = feed(&mut live, &mut fake, &audio);
        let committed: Vec<_> = steps.iter().filter(|s| matches!(s, Step::Committed(_))).collect();
        assert_eq!(committed.len(), 2, "{steps:?}");
        let calls_before_finish = fake.calls.len();
        let (text, lang) = live.finish(&mut fake, &audio).unwrap();
        assert_eq!(fake.calls.len(), calls_before_finish + 1, "seule la fin reste à transcrire");
        assert_eq!(text.split(' ').count(), 3);
        assert_eq!(lang, "fr");
        // le contexte du 2ᵉ morceau contient le vocabulaire et le texte du 1ᵉʳ
        let second_prompt = fake.calls[1].2.clone().unwrap();
        assert!(second_prompt.starts_with("Vocabulaire : Murmure. mot"), "{second_prompt}");
    }

    #[test]
    fn short_dictation_transcribed_in_one_piece() {
        // phrases avec pauses, mais moins de 10 s : un seul passage complet au relâchement, pour la justesse
        let audio = signal(&[(300, false), (2500, true), (900, false), (2500, true), (900, false), (1200, true)]);
        let mut fake = Fake::default();
        let mut live = Live::new(Some("fr".into()), None, true);
        let steps = feed(&mut live, &mut fake, &audio);
        assert!(steps.iter().all(|s| matches!(s, Step::Preview(_))), "{steps:?}");
        let finals_before = fake.calls.iter().filter(|c| !c.1).count();
        let (text, _) = live.finish(&mut fake, &audio).unwrap();
        let finals: Vec<_> = fake.calls.iter().filter(|c| !c.1).collect();
        assert_eq!((finals_before, finals.len()), (0, 1));
        assert_eq!(finals[0].0, audio.len());
        assert_eq!(text, "mot83");
    }

    #[test]
    fn previews_stay_short_during_long_dictation() {
        // 20 s de parole avec de petites pauses : l'aperçu ne recalcule que les dernières secondes
        let mut parts = vec![(300, false)];
        for _ in 0..5 {
            parts.extend([(3500, true), (450, false)]);
        }
        let audio = signal(&parts);
        let mut fake = Fake::default();
        let mut live = Live::new(Some("fr".into()), None, true);
        let steps = feed(&mut live, &mut fake, &audio);
        let longest = fake.calls.iter().filter(|c| c.1).map(|c| c.0).max().unwrap();
        assert!(longest <= 9 * SAMPLE_RATE, "aperçu de {} s", longest / SAMPLE_RATE);
        let Some(Step::Preview(last)) = steps.iter().rev().find(|s| matches!(s, Step::Preview(_))) else { panic!() };
        assert!(last.split(' ').count() >= 4, "aperçu figé + fin : {last}");
        // le texte final reste transcrit avec tout le contexte
        let finals_before = fake.calls.iter().filter(|c| !c.1).count();
        live.finish(&mut fake, &audio).unwrap();
        let finals: Vec<_> = fake.calls.iter().filter(|c| !c.1).map(|c| c.0).collect();
        assert!(finals.len() - finals_before == 1 && finals.iter().sum::<usize>() >= audio.len() - SAMPLE_RATE);
    }

    #[test]
    fn preview_while_speaking() {
        let audio = signal(&[(300, false), (2500, true), (900, false), (3000, true)]);
        let mut fake = Fake::default();
        let short = Config { min_silence_ms: 600, min_segment_ms: 2000, max_segment_ms: 24_000 };
        let mut live = Live::with_config(None, None, true, short);
        let steps = feed(&mut live, &mut fake, &audio);
        let previews: Vec<_> =
            steps.iter().filter_map(|s| if let Step::Preview(t) = s { Some(t) } else { None }).collect();
        assert!(previews.len() >= 2, "{steps:?}");
        assert!(previews.last().unwrap().split(' ').count() == 2, "définitif + provisoire : {previews:?}");
        assert!(fake.calls.iter().any(|c| c.1), "passe rapide utilisée");
    }

    #[test]
    fn silence_gives_nothing_and_hallucinations_dropped() {
        let mut fake =
            Fake { reply: Some("Sous-titres réalisés par la communauté d'Amara.org".into()), ..Default::default() };
        let audio = signal(&[(800, true)]);
        let (text, _) = Live::new(None, None, true).finish(&mut fake, &audio).unwrap();
        assert_eq!(text, "");

        let mut fake = Fake::default();
        let silence = signal(&[(6000, false)]);
        let mut live = Live::new(None, None, true);
        feed(&mut live, &mut fake, &silence);
        let (text, _) = live.finish(&mut fake, &silence).unwrap();
        assert!(fake.calls.is_empty(), "{:?}", fake.calls);
        assert_eq!(text, "");
    }

    #[test]
    fn context_tail_cut_on_word() {
        assert_eq!(tail("un deux trois", 50), "un deux trois");
        assert_eq!(tail("un deux trois quatre", 9), "quatre");
        assert_eq!(tail("", 9), "");
    }
}
