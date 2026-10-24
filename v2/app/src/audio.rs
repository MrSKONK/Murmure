//! Micro : capture en 16 kHz mono, volume instantané pour la pastille.
//! Le flux audio vit dans son propre fil (il ne peut pas toujours changer de fil sous Windows).
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use murmure_core::resample::Resampler;
use murmure_core::segment::SAMPLE_RATE;

#[derive(Default)]
pub struct Shared {
    pub samples: Mutex<Vec<f32>>,
    level: AtomicU32,
    pub recording: AtomicBool,
}

impl Shared {
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

#[derive(Default)]
pub struct Recorder {
    pub shared: Arc<Shared>,
    stop: Option<mpsc::Sender<()>>,
    done: Option<thread::JoinHandle<()>>,
}

fn find_device(host: &cpal::Host, name: &str) -> Option<cpal::Device> {
    if !name.is_empty() {
        if let Ok(mut devices) = host.input_devices() {
            if let Some(d) = devices.find(|d| d.name().is_ok_and(|n| n == name)) {
                return Some(d);
            }
        }
    }
    host.default_input_device() // micro choisi introuvable : micro par défaut
}

fn open(shared: Arc<Shared>, mic: &str) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = find_device(&host, mic).ok_or("aucun micro trouvé")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let mut resampler = Resampler::new(config.sample_rate.0, config.channels, SAMPLE_RATE as u32);
    let mut out = Vec::new();
    let mut convert = move |data: &[f32]| {
        if !shared.recording.load(Ordering::Relaxed) {
            return;
        }
        out.clear();
        resampler.push(data, &mut out);
        if out.is_empty() {
            return;
        }
        let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
        shared.level.store(rms.to_bits(), Ordering::Relaxed);
        if let Ok(mut samples) = shared.samples.lock() {
            samples.extend_from_slice(&out);
        }
    };
    let err = |e| eprintln!("[Micro] {e}");
    let stream = match format {
        cpal::SampleFormat::F32 => device.build_input_stream(&config, move |d: &[f32], _: &_| convert(d), err, None),
        cpal::SampleFormat::I16 => {
            let mut buf = Vec::new();
            device.build_input_stream(
                &config,
                move |d: &[i16], _: &_| {
                    buf.clear();
                    buf.extend(d.iter().map(|&s| f32::from(s) / 32768.0));
                    convert(&buf);
                },
                err,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut buf = Vec::new();
            device.build_input_stream(
                &config,
                move |d: &[u16], _: &_| {
                    buf.clear();
                    buf.extend(d.iter().map(|&s| (f32::from(s) - 32768.0) / 32768.0));
                    convert(&buf);
                },
                err,
                None,
            )
        }
        other => return Err(format!("format audio non pris en charge : {other}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

impl Recorder {
    pub fn start(&mut self, mic: &str) -> Result<(), String> {
        self.stop();
        self.shared.samples.lock().unwrap().clear();
        self.shared.level.store(0, Ordering::Relaxed);
        self.shared.recording.store(true, Ordering::Relaxed);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let shared = self.shared.clone();
        let mic = mic.to_string();
        let handle = thread::spawn(move || match open(shared, &mic) {
            Ok(stream) => {
                let _ = ready_tx.send(Ok(()));
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        });
        match ready_rx.recv().unwrap_or_else(|_| Err("fil audio arrêté".into())) {
            Ok(()) => {
                self.stop = Some(stop_tx);
                self.done = Some(handle);
                Ok(())
            }
            Err(e) => {
                self.shared.recording.store(false, Ordering::Relaxed);
                let _ = handle.join();
                Err(e)
            }
        }
    }

    /// Arrête l'enregistrement ; l'audio reste dans `shared.samples`.
    pub fn stop(&mut self) {
        self.shared.recording.store(false, Ordering::Relaxed);
        self.shared.level.store(0, Ordering::Relaxed);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(done) = self.done.take() {
            let _ = done.join();
        }
    }
}

pub fn list_microphones() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Ok(devices) = cpal::default_host().input_devices() {
        for name in devices.filter_map(|d| d.name().ok()) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}
