//! Conversion du micro (48 kHz, stéréo…) vers le 16 kHz mono attendu par Whisper, au fil de l'eau.
//! Chaque échantillon de sortie est la moyenne de la fenêtre d'entrée qu'il couvre (filtre anti-repliement
//! simple, suffisant pour la voix).
pub struct Resampler {
    ratio: f64,
    channels: usize,
    pending: Vec<f32>,
    pos: f64,
}

impl Resampler {
    pub fn new(input_rate: u32, channels: u16, output_rate: u32) -> Self {
        Resampler {
            ratio: f64::from(input_rate) / f64::from(output_rate),
            channels: usize::from(channels.max(1)),
            pending: Vec::new(),
            pos: 0.0,
        }
    }

    /// Ajoute des échantillons entrelacés et renvoie ceux qui sont prêts en sortie.
    pub fn push(&mut self, interleaved: &[f32], out: &mut Vec<f32>) {
        let ch = self.channels;
        self.pending.extend(interleaved.chunks_exact(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
        if self.ratio == 1.0 {
            out.append(&mut self.pending);
            return;
        }
        while self.pos + self.ratio <= self.pending.len() as f64 {
            let start = self.pos.floor() as usize;
            let end = ((self.pos + self.ratio).ceil() as usize).clamp(start + 1, self.pending.len());
            let window = &self.pending[start..end];
            out.push(window.iter().sum::<f32>() / window.len() as f32);
            self.pos += self.ratio;
        }
        let consumed = (self.pos.floor() as usize).min(self.pending.len());
        self.pending.drain(..consumed);
        self.pos -= consumed as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_and_channels() {
        for (rate, channels) in [(48_000, 2), (44_100, 1), (16_000, 1), (16_000, 2), (8_000, 1)] {
            let mut r = Resampler::new(rate, channels, 16_000);
            let mut out = Vec::new();
            // une seconde envoyée par petits paquets, comme le fait le micro
            let input: Vec<f32> = (0..rate as usize * usize::from(channels)).map(|i| (i % 7) as f32 * 0.1).collect();
            for chunk in input.chunks(480 * usize::from(channels)) {
                r.push(chunk, &mut out);
            }
            assert!((out.len() as i64 - 16_000).abs() <= 2, "{rate} Hz × {channels} : {}", out.len());
        }
    }

    #[test]
    fn constant_signal_kept() {
        let mut r = Resampler::new(44_100, 2, 16_000);
        let mut out = Vec::new();
        r.push(&vec![0.5; 44_100 * 2], &mut out);
        assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-6));
    }
}
