//! Isolement de la voix : réduction du bruit et de la musique de fond avec RNNoise (réseau de neurones léger,
//! sans modèle à télécharger), au fil de l'eau, du micro jusqu'au 16 kHz mono attendu par Whisper.
//!
//! Mesuré sur voix + musique d'accompagnement : à volume égal avec la voix, le taux d'erreur de mots passe de
//! 33 % à 10 % (celui de la voix seule) ; voix seule ou musique discrète, rien ne change.
use nnnoiseless::{DenoiseState, FRAME_SIZE};

use crate::resample::Resampler;

/// RNNoise travaille en 48 kHz, par blocs de 10 ms.
const RATE: u32 = 48_000;
/// RNNoise attend des échantillons à l'échelle d'un entier 16 bits.
const SCALE: f32 = 32_768.0;

pub struct Denoiser {
    up: Resampler,
    down: Resampler,
    state: Box<DenoiseState<'static>>,
    /// échantillons 48 kHz en attente d'un bloc complet
    pending: Vec<f32>,
    cleaned: Vec<f32>,
    /// le premier bloc rendu par RNNoise est un fondu d'entrée : écarté
    warming: bool,
}

impl Denoiser {
    /// `input_rate`, `channels` : format du micro. La sortie est en `output_rate`, mono.
    pub fn new(input_rate: u32, channels: u16, output_rate: u32) -> Self {
        Denoiser {
            up: Resampler::new(input_rate, channels, RATE),
            down: Resampler::new(RATE, 1, output_rate),
            state: DenoiseState::new(),
            pending: Vec::new(),
            cleaned: Vec::new(),
            warming: true,
        }
    }

    /// Ajoute des échantillons du micro ; ajoute à `out` la voix débruitée (au 16 kHz mono) déjà disponible.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.up.push(input, &mut self.pending);
        self.cleaned.clear();
        let mut done = 0;
        let mut frame = [0.0f32; FRAME_SIZE];
        let mut block = [0.0f32; FRAME_SIZE];
        while self.pending.len() - done >= FRAME_SIZE {
            for (dst, src) in block.iter_mut().zip(&self.pending[done..done + FRAME_SIZE]) {
                *dst = src * SCALE;
            }
            self.state.process_frame(&mut frame, &block);
            if self.warming {
                self.warming = false;
            } else {
                self.cleaned.extend(frame.iter().map(|s| s / SCALE));
            }
            done += FRAME_SIZE;
        }
        self.pending.drain(..done);
        self.down.push(&self.cleaned, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_follows_input_length() {
        // micro 44,1 kHz stéréo, par paquets irréguliers comme ceux d'un vrai micro
        let mut d = Denoiser::new(44_100, 2, 16_000);
        let (mut total_in, mut out) = (0usize, Vec::new());
        for n in [441 * 2, 1000 * 2, 4410 * 2, 333 * 2, 44_100 * 2] {
            let chunk: Vec<f32> = (0..n).map(|i| ((i as f32) * 0.01).sin() * 0.3).collect();
            d.push(&chunk, &mut out);
            total_in += n / 2;
        }
        let expected = total_in as f64 * 16_000.0 / 44_100.0;
        assert!((out.len() as f64 - expected).abs() < 1_000.0, "{} échantillons pour {expected:.0} attendus", out.len());
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.5));
    }

    #[test]
    fn silence_stays_silent() {
        let mut d = Denoiser::new(16_000, 1, 16_000);
        let mut out = Vec::new();
        d.push(&vec![0.0; 16_000], &mut out);
        assert!(!out.is_empty());
        assert!(out.iter().all(|s| s.abs() < 1e-3));
    }
}
