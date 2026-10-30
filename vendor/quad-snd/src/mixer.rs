use crate::{AudioContext, PlaySoundParams};

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;

#[cfg(test)]
mod loop_tests {
    use super::*;
    #[test]
    fn loop_continues_at_the_current_buffer_offset() {
        let (builder, control) = Mixer::new();
        let mut mixer = builder.build();
        control.send(AudioMessage::AddSound(
            0,
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6].into(),
        ));
        control.send(AudioMessage::Play(0, 0, true, 1.));
        control.flush_decoder();
        let mut samples = [0.; 8];
        mixer.fill_audio_buffer(&mut samples, 4);
        assert_eq!(samples, [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.1, 0.2]);
        mixer.fill_audio_buffer(&mut samples, 4);
        assert_eq!(samples, [0.3, 0.4, 0.5, 0.6, 0.1, 0.2, 0.3, 0.4]);
    }
    #[test]
    fn empty_loop_does_not_hang_the_audio_callback() {
        let (builder, control) = Mixer::new();
        let mut mixer = builder.build();
        control.send(AudioMessage::AddSound(0, vec![].into()));
        control.send(AudioMessage::Play(0, 0, true, 1.));
        control.flush_decoder();
        let mut samples = [1.; 8];
        mixer.fill_audio_buffer(&mut samples, 4);
        assert_eq!(samples, [0.; 8]);
    }
    #[test]
    fn overlapping_loud_cues_are_limited_without_clipping() {
        let (builder, control) = Mixer::new();
        let mut mixer = builder.build();
        control.send(AudioMessage::AddSound(0, vec![0.8, -0.8, 0.8, -0.8].into()));
        control.send(AudioMessage::Play(0, 0, false, 1.));
        control.send(AudioMessage::Play(0, 1, false, 1.));
        control.flush_decoder();
        let mut samples = [0.; 4];
        mixer.fill_audio_buffer(&mut samples, 2);
        assert!(samples.iter().all(|v| v.abs() <= 1. && v.abs() > 0.95));
    }

    fn wav() -> Vec<u8> {
        let mut bytes = Vec::from(*b"RIFF");
        bytes.extend(40u32.to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(44100u32.to_le_bytes());
        bytes.extend(176400u32.to_le_bytes());
        bytes.extend(4u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(4u32.to_le_bytes());
        bytes.extend(8192i16.to_le_bytes());
        bytes.extend((-8192i16).to_le_bytes());
        bytes
    }

    #[test]
    fn load_and_play_queue_without_waiting_for_decoder() {
        let (decoded, rx) = mpsc::channel();
        let (tx, commands) = mpsc::channel();
        let control = MixerControl {
            tx,
            sound_id: Cell::new(0),
            play_id: Cell::new(0),
        };
        let mut mixer = MixerBuilder { rx }.build();
        // No decoder exists yet. These operations must return without its help.
        let id = control.load(&wav());
        control.play(
            id,
            PlaySoundParams {
                looped: true,
                volume: 1.,
            },
        );
        let worker = std::thread::spawn(move || decode_commands(commands, decoded));
        control.flush_decoder();
        let mut samples = [0.; 4];
        mixer.fill_audio_buffer(&mut samples, 2);
        assert_eq!(samples, [0.25, -0.25, 0.25, -0.25]);
        drop(control);
        worker.join().unwrap();
    }

    #[test]
    fn pending_load_preserves_stop_delete_and_survives_invalid_audio() {
        let (builder, control) = Mixer::new();
        let mut mixer = builder.build();
        let bad = control.load(b"invalid audio");
        control.play(bad, PlaySoundParams::default());
        control.delete(bad);
        let stopped = control.load(&wav());
        control.play(
            stopped,
            PlaySoundParams {
                looped: true,
                volume: 1.,
            },
        );
        control.stop_all(stopped);
        let deleted = control.load(&wav());
        control.play(
            deleted,
            PlaySoundParams {
                looped: true,
                volume: 1.,
            },
        );
        control.delete(deleted);
        control.flush_decoder();
        let mut samples = [1.; 4];
        mixer.fill_audio_buffer(&mut samples, 2);
        assert_eq!(samples, [0.; 4]);
        assert_eq!(mixer.sounds.len(), 1);
        assert!(mixer.sounds.contains_key(&stopped));
        assert!(mixer.mixer_state.is_empty());
        control.play(
            stopped,
            PlaySoundParams {
                looped: true,
                volume: 0.5,
            },
        );
        control.flush_decoder();
        mixer.fill_audio_buffer(&mut samples, 2);
        assert_eq!(samples, [0.125, -0.125, 0.125, -0.125]);
    }
}

enum AudioMessage {
    AddSound(u32, Arc<[f32]>),
    Play(u32, u32, bool, f32),
    Stop(u32),
    StopAll(u32),
    SetVolume(u32, f32),
    SetVolumeAll(u32, f32),
    Delete(u32),
}

enum ControlMessage {
    Decode(u32, Vec<u8>),
    Forward(AudioMessage),
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}

// Decode and allocate shared sample storage away from both the UI and the real-time
// callback. A single FIFO also keeps load/play/stop/delete in their original order.
fn decode_commands(rx: mpsc::Receiver<ControlMessage>, tx: mpsc::Sender<AudioMessage>) {
    while let Ok(command) = rx.recv() {
        let message = match command {
            ControlMessage::Decode(id, bytes) => AudioMessage::AddSound(
                id,
                load_samples_from_file(&bytes).unwrap_or_default().into(),
            ),
            ControlMessage::Forward(message) => message,
            #[cfg(test)]
            ControlMessage::Barrier(done) => {
                let _ = done.send(());
                continue;
            }
        };
        if tx.send(message).is_err() {
            break;
        }
    }
}

#[derive(Debug)]
pub struct SoundState {
    sound_id: u32,
    play_id: u32,
    sample: usize,
    data: Arc<[f32]>,
    looped: bool,
    volume: f32,
}

impl SoundState {
    fn get_samples(&mut self, n: usize) -> &[f32] {
        let data = &self.data[self.sample..];

        self.sample += n;

        match data.get(..n) {
            Some(data) => data,
            None => data,
        }
    }

    fn rewind(&mut self) {
        self.sample = 0;
    }
}

pub struct Mixer {
    rx: mpsc::Receiver<AudioMessage>,
    sounds: HashMap<u32, Arc<[f32]>>,
    mixer_state: Vec<SoundState>,
}

pub struct MixerBuilder {
    rx: mpsc::Receiver<AudioMessage>,
}

pub struct MixerControl {
    tx: mpsc::Sender<ControlMessage>,
    sound_id: Cell<u32>,
    play_id: Cell<u32>,
}

pub struct Playback {
    play_id: u32,
}

impl Playback {
    pub fn stop(self, ctx: &AudioContext) {
        ctx.mixer_ctrl.send(AudioMessage::Stop(self.play_id));
    }

    pub fn set_volume(&self, ctx: &AudioContext, volume: f32) {
        ctx.mixer_ctrl
            .send(AudioMessage::SetVolume(self.play_id, volume));
    }
}

impl MixerControl {
    pub fn load(&self, data: &[u8]) -> u32 {
        let sound_id = self.sound_id.get();

        self.tx
            .send(ControlMessage::Decode(sound_id, data.to_owned()))
            .unwrap_or_else(|_| println!("Audio thread died"));
        self.sound_id.set(sound_id + 1);

        sound_id
    }

    pub fn play(&self, sound_id: u32, params: PlaySoundParams) -> Playback {
        let play_id = self.play_id.get();

        self.send(AudioMessage::Play(
            sound_id,
            play_id,
            params.looped,
            params.volume,
        ));

        self.play_id.set(play_id + 1);

        Playback { play_id }
    }

    pub fn stop(&self, play_id: u32) {
        self.send(AudioMessage::Stop(play_id));
    }

    pub fn stop_all(&self, sound_id: u32) {
        self.send(AudioMessage::StopAll(sound_id));
    }

    pub fn set_volume_all(&self, sound_id: u32, volume: f32) {
        self.send(AudioMessage::SetVolumeAll(sound_id, volume));
    }

    pub fn delete(&self, sound_id: u32) {
        self.send(AudioMessage::Delete(sound_id));
    }

    fn send(&self, message: AudioMessage) {
        self.tx
            .send(ControlMessage::Forward(message))
            .unwrap_or_else(|_| println!("Audio thread died"))
    }

    #[cfg(test)]
    fn flush_decoder(&self) {
        let (done, wait) = mpsc::channel();
        self.tx.send(ControlMessage::Barrier(done)).unwrap();
        wait.recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }
}

impl MixerBuilder {
    pub fn build(self) -> Mixer {
        Mixer {
            rx: self.rx,
            sounds: HashMap::new(),
            mixer_state: vec![],
        }
    }
}

impl Mixer {
    pub fn new() -> (MixerBuilder, MixerControl) {
        let (decoded, rx) = mpsc::channel();
        let (tx, commands) = mpsc::channel();
        std::thread::Builder::new()
            .name("audio-decode".into())
            .spawn(move || decode_commands(commands, decoded))
            .expect("audio decoder thread");

        (
            MixerBuilder { rx },
            MixerControl {
                tx,
                sound_id: Cell::new(0),
                play_id: Cell::new(0),
            },
        )
    }

    pub fn fill_audio_buffer(&mut self, buffer: &mut [f32], frames: usize) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                AudioMessage::AddSound(id, data) => {
                    self.sounds.insert(id, data);
                }
                AudioMessage::Play(sound_id, play_id, looped, volume) => {
                    if let Some(data) = self.sounds.get(&sound_id).filter(|data| !data.is_empty()) {
                        self.mixer_state.push(SoundState {
                            sound_id,
                            play_id,
                            sample: 0,
                            data: data.clone(),
                            looped,
                            volume,
                        });
                    }
                }
                AudioMessage::Stop(play_id) => {
                    if let Some(i) = self.mixer_state.iter().position(|s| s.play_id == play_id) {
                        self.mixer_state.swap_remove(i);
                    }
                }
                AudioMessage::StopAll(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                }
                AudioMessage::SetVolume(play_id, volume) => {
                    if let Some(sound) = self.mixer_state.iter_mut().find(|s| s.play_id == play_id)
                    {
                        sound.volume = volume;
                    }
                }
                AudioMessage::SetVolumeAll(sound_id, volume) => {
                    for sound in self
                        .mixer_state
                        .iter_mut()
                        .filter(|s| s.sound_id == sound_id)
                    {
                        sound.volume = volume;
                    }
                }
                AudioMessage::Delete(sound_id) => {
                    for i in (0..self.mixer_state.len()).rev() {
                        if self.mixer_state[i].sound_id == sound_id {
                            self.mixer_state.swap_remove(i);
                        }
                    }
                    self.sounds.remove(&sound_id);
                }
            }
        }

        // zeroize the buffer
        buffer.fill(0.0);

        // Note: Doing manual iteration so we can remove sounds that finished playing
        let mut i = 0;

        while let Some(sound) = self.mixer_state.get_mut(i) {
            let volume = sound.volume;
            let mut remainder = buffer.len();

            loop {
                let samples = sound.get_samples(remainder);

                let offset = buffer.len() - remainder;
                for (b, s) in buffer[offset..].iter_mut().zip(samples) {
                    *b += s * volume;
                }

                remainder -= samples.len();

                if remainder > 0 && sound.looped {
                    sound.rewind();
                    continue;
                }

                break;
            }

            if remainder > 0 {
                self.mixer_state.swap_remove(i);
            } else {
                i += 1;
            }
        }
        // Preserve transient detail at normal levels while preventing overlapping
        // full-volume cues from clipping the device output.
        for sample in buffer.iter_mut() {
            if sample.abs() > 0.95 {
                *sample = sample.signum() * (0.95 + 0.05 * ((sample.abs() - 0.95) / 0.05).tanh());
            }
        }
    }
}

/// Parse ogg/wav/etc and get  resampled to 44100, 2 channel data
pub fn load_samples_from_file(bytes: &[u8]) -> Result<Vec<f32>, ()> {
    let mut audio_stream = {
        let file = std::io::Cursor::new(bytes);
        audrey::Reader::new(file).map_err(|_| ())?
    };

    let description = audio_stream.description();
    let channels_count = description.channel_count();
    if !(channels_count == 1 || channels_count == 2) || description.sample_rate() == 0 {
        return Err(());
    }

    let mut frames: Vec<f32> = Vec::with_capacity(4096);
    for sample in audio_stream.samples::<f32>() {
        let sample = sample.map_err(|_| ())?;
        frames.push(sample);
        if channels_count == 1 {
            frames.push(sample);
        }
    }
    // Never pass an incomplete stereo frame to the resampler or mixer.
    if frames.len() % 2 != 0 {
        return Err(());
    }

    let sample_rate = description.sample_rate();

    // stupid nearest-neighbor resampler
    if sample_rate != 44100 {
        let mut new_length = ((44100 as f32 / sample_rate as f32) * frames.len() as f32) as usize;

        // `new_length` must be an even number
        new_length -= new_length % 2;

        let mut resampled = vec![0.0; new_length];

        for (n, sample) in resampled.chunks_exact_mut(2).enumerate() {
            let ix = 2 * ((n as f32 / new_length as f32) * frames.len() as f32) as usize;
            sample[0] = frames[ix];
            sample[1] = frames[ix + 1];
        }
        return Ok(resampled);
    }

    Ok(frames)
}
