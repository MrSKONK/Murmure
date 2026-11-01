//! Conversion du micro (48 kHz, stéréo…) vers le 16 kHz mono attendu par Whisper, au fil de l'eau.
//! Filtre passe-bas en sinus cardinal fenêtré : les aigus au-delà de 8 kHz sont retirés avant la réduction,
//! sinon ils se replient en sifflements parasites dans la bande de la voix et gênent la reconnaissance.
use std::f64::consts::PI;

pub struct Resampler {
    ratio: f64,
    channels: usize,
    /// échantillons mono en attente, précédés de `half` échantillons d'historique
    pending: Vec<f32>,
    pos: f64,
    half: usize,
    cutoff: f64,
}

impl Resampler {
    pub fn new(input_rate: u32, channels: u16, output_rate: u32) -> Self {
        let ratio = f64::from(input_rate) / f64::from(output_rate);
        let half = (16.0 * ratio.max(1.0)).ceil() as usize;
        Resampler {
            ratio,
            channels: usize::from(channels.max(1)),
            pending: vec![0.0; half],
            pos: half as f64,
            half,
            // fréquence de coupure (relative à l'entrée) un peu sous la moitié de la fréquence de sortie
            cutoff: if ratio > 1.0 { 0.95 / ratio } else { 1.0 },
        }
    }

    fn kernel(&self, d: f64) -> f64 {
        let x = d / self.half as f64;
        if x.abs() >= 1.0 {
            return 0.0;
        }
        let window = 0.42 + 0.5 * (PI * x).cos() + 0.08 * (2.0 * PI * x).cos(); // Blackman
        let t = self.cutoff * d;
        let sinc = if t.abs() < 1e-9 { 1.0 } else { (PI * t).sin() / (PI * t) };
        sinc * window
    }

    /// Ajoute des échantillons entrelacés et renvoie ceux qui sont prêts en sortie.
    pub fn push(&mut self, interleaved: &[f32], out: &mut Vec<f32>) {
        let ch = self.channels;
        let mono = interleaved.chunks_exact(ch).map(|f| f.iter().sum::<f32>() / ch as f32);
        if self.ratio == 1.0 {
            out.extend(mono);
            return;
        }
        self.pending.extend(mono);
        let half = self.half as isize;
        while self.pos + self.half as f64 + 1.0 < self.pending.len() as f64 {
            let center = self.pos.floor() as isize;
            let (mut acc, mut norm) = (0.0f64, 0.0f64);
            for n in (center - half + 1)..=(center + half) {
                let w = self.kernel(self.pos - n as f64);
                acc += w * f64::from(self.pending[n as usize]);
                norm += w;
            }
            out.push((acc / norm) as f32);
            self.pos += self.ratio;
        }
        // garde l'historique nécessaire au prochain échantillon
        let keep_from = (self.pos.floor() as usize).saturating_sub(self.half);
        self.pending.drain(..keep_from);
        self.pos -= keep_from as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(rate: u32, channels: u16, input: &[f32]) -> Vec<f32> {
        let mut r = Resampler::new(rate, channels, 16_000);
        let mut out = Vec::new();
        for chunk in input.chunks(480 * usize::from(channels)) {
            r.push(chunk, &mut out); // par petits paquets, comme le fait le micro
        }
        out
    }

    fn tone(rate: u32, freq: f64, secs: f64) -> Vec<f32> {
        (0..(f64::from(rate) * secs) as usize)
            .map(|i| (2.0 * PI * freq * i as f64 / f64::from(rate)).sin() as f32)
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        let x = &x[x.len() / 10..]; // sans le démarrage du filtre
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn rates_and_channels() {
        for (rate, channels) in [(48_000, 2), (44_100, 1), (16_000, 1), (16_000, 2), (8_000, 1)] {
            let input: Vec<f32> = (0..rate as usize * usize::from(channels)).map(|i| (i % 7) as f32 * 0.1).collect();
            let out = run(rate, channels, &input);
            assert!((out.len() as i64 - 16_000).abs() <= 120, "{rate} Hz × {channels} : {}", out.len());
        }
    }

    #[test]
    fn voice_kept_and_high_frequencies_removed() {
        for rate in [48_000, 44_100] {
            let voice = rms(&run(rate, 1, &tone(rate, 1000.0, 1.0)));
            assert!((voice - 0.707).abs() < 0.03, "{rate} Hz, 1 kHz : {voice}");
            let hiss = rms(&run(rate, 1, &tone(rate, 11_000.0, 1.0)));
            assert!(hiss < 0.01, "{rate} Hz, 11 kHz : {hiss}");
        }
    }

    #[test]
    fn constant_signal_kept() {
        let out = run(44_100, 2, &vec![0.5; 44_100 * 2]);
        assert!(out[out.len() / 10..].iter().all(|&s| (s - 0.5).abs() < 1e-3));
    }
}
