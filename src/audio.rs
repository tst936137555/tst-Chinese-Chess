//! Original, deterministic pentatonic score and wood-piece Foley. No downloaded recordings.
use crate::storage::Settings;
use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};
const RATE: usize = 44_100;
const TAU: f32 = std::f32::consts::TAU;

type AudioAsset = (Option<(usize, usize)>, Vec<u8>);

pub struct AudioBank {
    music: Option<Sound>,
    effects: Vec<Vec<Option<Sound>>>,
    music_started: bool,
    current: f32,
    duck_until: f64,
    enabled: bool,
    loading: std::sync::mpsc::Receiver<AudioAsset>,
    loading_started: std::time::Instant,
    loading_complete: bool,
    pending_effects: usize,
    music_start: Option<std::sync::mpsc::Sender<()>>,
}
impl AudioBank {
    pub async fn load() -> Self {
        let (tx, loading) = std::sync::mpsc::sync_channel(2);
        let (music_start, music_gate) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for kind in 0..6 {
                let mono = effect_samples(kind);
                for position in 0..if kind < 3 { 9 } else { 1 } {
                    let pan = if kind < 3 {
                        (position as f32 - 4.) / 4. * 0.7
                    } else {
                        0.
                    };
                    if tx
                        .send((Some((kind, position)), wav(&stereo(&mono, pan))))
                        .is_err()
                    {
                        return;
                    }
                }
            }
            if music_gate.recv().is_ok() {
                let _ = tx.send((None, wav(&music_samples())));
            }
        });
        Self {
            music: None,
            effects: (0..6)
                .map(|k| vec![None; if k < 3 { 9 } else { 1 }])
                .collect(),
            music_started: false,
            current: 0.,
            duck_until: 0.,
            enabled: true,
            loading,
            loading_started: std::time::Instant::now(),
            loading_complete: false,
            pending_effects: 30,
            music_start: Some(music_start),
        }
    }
    pub async fn prepare(&mut self) {
        if self.loading_complete {
            return;
        }
        let (target, bytes) = match self.loading.try_recv() {
            Ok(asset) => asset,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.loading_complete = true;
                self.pending_effects = 0;
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        };
        {
            let sound = match load_sound_from_bytes(&bytes).await {
                Ok(s) => Some(s),
                Err(e) => {
                    crate::diagnostics::failure(
                        crate::diagnostics::FailureKind::Assets,
                        "audio",
                        &e.to_string(),
                    );
                    None
                }
            };
            if let Some((kind, index)) = target {
                self.effects[kind][index] = sound;
                self.pending_effects = self.pending_effects.saturating_sub(1);
            } else {
                self.loading_complete = true;
                self.music = sound;
                crate::diagnostics::measure(
                    "audio_queued_ms",
                    self.loading_started.elapsed().as_secs_f64() * 1000.,
                );
            }
        }
    }
    pub fn suspend(&mut self) {
        if let Some(m) = &self.music {
            stop_sound(m);
        }
        for sound in self.effects.iter().flatten().flatten() {
            stop_sound(sound);
        }
        self.music_started = false;
        self.current = 0.;
    }
    pub fn needs_tick(&self, settings: &Settings, now: f64) -> bool {
        let target = if self.music.is_some() && settings.sound && settings.music_enabled {
            settings.music_volume * if now < self.duck_until { 0.42 } else { 1. }
        } else {
            0.
        };
        self.pending_effects > 0
            || (!self.loading_complete && self.music_start.is_none())
            || (self.current - target).abs() > 0.001
            || now < self.duck_until
    }
    pub fn tick(&mut self, s: &Settings, dt: f32, now: f64) {
        if crate::platform::suspended() {
            return;
        }
        if s.sound
            && s.music_enabled
            && s.music_volume > 0.
            && let Some(start) = self.music_start.take()
        {
            self.loading_started = std::time::Instant::now();
            let _ = start.send(());
        }
        if let Some(music) = &self.music {
            let audible = s.sound && s.music_enabled && s.music_volume > 0.;
            if audible && !self.music_started {
                play_sound(
                    music,
                    PlaySoundParams {
                        looped: true,
                        volume: 0.,
                    },
                );
                self.music_started = true;
            }
            let target = if s.sound && s.music_enabled {
                s.music_volume.clamp(0., 1.) * if now < self.duck_until { 0.42 } else { 1. }
            } else {
                0.
            };
            self.current += (target - self.current) * (1. - (-dt.clamp(0., 0.1) / 0.3).exp());
            set_sound_volume(music, self.current);
            if !audible && self.music_started && self.current < 0.001 {
                stop_sound(music);
                self.music_started = false;
                self.current = 0.;
            }
        }
        if self.enabled && (!s.sound || !s.effects_enabled) {
            for sound in self.effects.iter().flatten().flatten() {
                stop_sound(sound);
            }
        }
        self.enabled = s.sound && s.effects_enabled;
    }
    pub fn play(&mut self, kind: usize, visible_file: i8, s: &Settings, now: f64) {
        if !s.sound || !s.effects_enabled {
            return;
        }
        let index = if kind < 3 {
            visible_file.clamp(0, 8) as usize
        } else {
            0
        };
        if let Some(Some(sound)) = self.effects.get(kind).and_then(|bank| bank.get(index)) {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume: s.effects_volume.clamp(0., 1.),
                },
            );
            self.duck_until = now + if kind >= 3 { 1.9 } else { 0.55 };
        }
    }
}
fn frequency(midi: u8) -> f32 {
    440. * 2_f32.powf((midi as f32 - 69.) / 12.)
}
fn envelope(t: f32, duration: f32, decay: f32) -> f32 {
    if t < 0. || t >= duration {
        return 0.;
    }
    (1. - (-t / 0.004).exp()) * (-t * decay).exp() * ((duration - t) / 0.045).clamp(0., 1.)
}
fn pluck(t: f32, midi: u8, duration: f32) -> f32 {
    let f = frequency(midi);
    let phase = TAU * f * t;
    (phase.sin()
        + 0.33 * (phase * 2.003).sin() * (-t * 2.).exp()
        + 0.12 * (phase * 3.997).sin() * (-t * 5.).exp())
        * envelope(t, duration, 2.4)
}
fn noise(i: usize) -> f32 {
    let mut n = (i as u32).wrapping_mul(747796405).wrapping_add(2891336453);
    n = ((n >> ((n >> 28) + 4)) ^ n).wrapping_mul(277803737);
    (((n >> 22) ^ n) as f32 / u32::MAX as f32) * 2. - 1.
}
pub fn effect_samples(kind: usize) -> Vec<f32> {
    let duration = match kind {
        0 => 0.18,
        1 => 0.30,
        2 => 0.72,
        3 => 1.9,
        4 => 1.7,
        _ => 1.7,
    };
    let mut out = vec![0.; (duration * RATE as f32) as usize];
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let tap = |time: f32, seed: usize| {
            if time >= 0. {
                (0.7 * (TAU * 185. * time).sin() * (-time * 35.).exp()
                    + 0.32 * (TAU * 720. * time).sin() * (-time * 63.).exp()
                    + noise(i + seed) * (-time * 180.).exp() * 0.6)
                    * envelope(time, 0.17, 0.)
            } else {
                0.
            }
        };
        *sample = match kind {
            0 => tap(t, 0),
            1 => {
                tap(t, 37) * 0.8
                    + tap(t - 0.065, 991) * 0.62
                    + 0.12 * noise(i + 713) * envelope(t, 0.28, 16.)
            }
            2 => {
                tap(t, 1) * 0.38
                    + ((TAU * 587.33 * t).sin() + 0.21 * (TAU * 1182. * t).sin())
                        * envelope(t, 0.7, 6.)
                        * 0.62
            }
            3 => [(0., 62), (0.16, 66), (0.32, 69), (0.52, 74)]
                .iter()
                .map(|&(at, n)| {
                    if t >= at {
                        pluck(t - at, n, 1.35) * 0.45
                    } else {
                        0.
                    }
                })
                .sum(),
            4 => [(0., 59), (0.22, 57), (0.47, 54), (0.73, 50)]
                .iter()
                .map(|&(at, n)| {
                    if t >= at {
                        pluck(t - at, n, 0.95) * 0.4
                    } else {
                        0.
                    }
                })
                .sum(),
            _ => [(0., 62), (0.18, 69), (0.40, 76)]
                .iter()
                .map(|&(at, n)| {
                    if t >= at {
                        pluck(t - at, n, 1.25) * 0.38
                    } else {
                        0.
                    }
                })
                .sum(),
        };
    }
    let peak = out.iter().map(|v| v.abs()).fold(0_f32, f32::max).max(0.001);
    for v in &mut out {
        *v = *v / peak * 0.72;
    }
    out
}
pub fn stereo(mono: &[f32], pan: f32) -> Vec<f32> {
    let pan = pan.clamp(-1., 1.);
    let left = ((1. - pan) * 0.5).sqrt();
    let right = ((1. + pan) * 0.5).sqrt();
    let delay = (RATE as f32 * 0.007) as usize;
    let mut out = vec![0.; (mono.len() + delay) * 2];
    for (i, &v) in mono.iter().enumerate() {
        out[i * 2] += v * left;
        out[i * 2 + 1] += v * right;
        out[(i + delay) * 2] += v * right * 0.085;
        out[(i + delay) * 2 + 1] += v * left * 0.085;
    }
    out
}
pub fn music_samples() -> Vec<f32> {
    let frames = RATE * 32;
    let mut out = vec![0.; frames * 2];
    // Eight four-beat bars at 60 BPM; original guqin-like plucks and breathy flute.
    let melody: [Option<u8>; 32] = [
        Some(74),
        None,
        Some(78),
        Some(76),
        Some(74),
        None,
        Some(69),
        None,
        Some(71),
        Some(74),
        Some(76),
        None,
        Some(78),
        None,
        Some(76),
        None,
        Some(74),
        None,
        Some(69),
        Some(71),
        Some(74),
        None,
        Some(78),
        None,
        Some(81),
        Some(78),
        Some(76),
        None,
        Some(74),
        None,
        None,
        None,
    ];
    let mut events: Vec<(f32, u8, f32, f32, bool)> = Vec::new();
    for bar in 0..8 {
        let base = if bar % 4 == 2 { 57 } else { 50 };
        events.push((bar as f32 * 4., base, 0.17, -0.32, false));
        events.push((bar as f32 * 4. + 2., base + 12, 0.09, 0.35, false));
    }
    for (i, note) in melody.iter().enumerate() {
        if let Some(n) = note {
            events.push((i as f32, *n, 0.085, 0.10, true));
            if i % 4 == 0 {
                events.push((i as f32 + 0.035, n - 12, 0.10, 0.42, false));
            }
        }
    }
    for (start, midi, gain, pan, flute) in events {
        let duration = if flute { 1.85 } else { 3.2 };
        let frequency = frequency(midi);
        let left = ((1. - pan) * 0.5).sqrt();
        let right = ((1. + pan) * 0.5).sqrt();
        for i in 0..(duration * RATE as f32) as usize {
            let t = i as f32 / RATE as f32;
            let value = if flute {
                let e = (t / 0.17).clamp(0., 1.) * ((duration - t) / 0.5).clamp(0., 1.);
                let phase = TAU * frequency * t + 0.025 * (TAU * 4.6 * t).sin();
                (phase.sin() + 0.13 * (phase * 2.).sin() + 0.018 * noise(i)) * e
            } else {
                pluck(t, midi, duration)
            } * gain;
            let index = ((start * RATE as f32) as usize + i) % frames;
            out[index * 2] += value * left;
            out[index * 2 + 1] += value * right;
            let reflected = (index + 4410) % frames;
            out[reflected * 2] += value * right * 0.14;
            out[reflected * 2 + 1] += value * left * 0.14;
        }
    }
    let peak = out.iter().map(|v| v.abs()).fold(0_f32, f32::max).max(0.001);
    for v in &mut out {
        *v = *v / peak * 0.32;
    }
    out
}
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let size = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(size as usize + 44);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(size + 36).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16_u32.to_le_bytes());
    b.extend_from_slice(&1_u16.to_le_bytes());
    b.extend_from_slice(&2_u16.to_le_bytes());
    b.extend_from_slice(&(RATE as u32).to_le_bytes());
    b.extend_from_slice(&(RATE as u32 * 4).to_le_bytes());
    b.extend_from_slice(&4_u16.to_le_bytes());
    b.extend_from_slice(&16_u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&size.to_le_bytes());
    for &s in samples {
        b.extend_from_slice(&((s.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
    }
    b
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stereo_tracks_follow_the_piece_and_remain_below_clipping() {
        let mono = effect_samples(0);
        let left = stereo(&mono, -0.7);
        let right = stereo(&mono, 0.7);
        let energy = |samples: &[f32], channel: usize| {
            samples
                .iter()
                .skip(channel)
                .step_by(2)
                .map(|v| v * v)
                .sum::<f32>()
        };
        assert!(energy(&left, 0) > energy(&left, 1) * 3.);
        assert!(energy(&right, 1) > energy(&right, 0) * 3.);
        assert!(
            left.iter()
                .chain(right.iter())
                .all(|v| v.is_finite() && v.abs() < 1.)
        );
        let bytes = wav(&left);
        assert_eq!(&bytes[22..24], &2_u16.to_le_bytes());
        assert_eq!(&bytes[24..28], &44_100_u32.to_le_bytes());
    }
    #[test]
    fn music_is_stereo_and_the_loop_boundary_has_no_discontinuous_click() {
        let music = music_samples();
        assert_eq!(music.len(), RATE * 32 * 2);
        assert!(music.iter().all(|v| v.is_finite() && v.abs() <= 0.321));
        assert!((music[0] - music[music.len() - 2]).abs() < 0.02);
        assert!((music[1] - music[music.len() - 1]).abs() < 0.02);
        assert!(
            music
                .as_chunks::<2>()
                .0
                .iter()
                .any(|v| (v[0] - v[1]).abs() > 0.001)
        );
    }
}
