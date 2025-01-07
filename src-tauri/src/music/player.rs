use rodio::{ Decoder, OutputStreamHandle, Sink, Source };
use std::{
    f32::consts::PI,
    fs::File,
    io::BufReader,
    sync::{ atomic::{ AtomicBool, Ordering }, Arc, Mutex },
    time::{ Instant, Duration },
};
use crate::{ db::types::Song, music::queue::Queue };
use serde::{ Deserialize, Serialize };

#[derive(Debug, Serialize, Deserialize)]
pub struct EQSettings {
    values: std::collections::HashMap<String, String>,
}

pub struct AudioPlayer {
    stream_handle: OutputStreamHandle,
    sink: Arc<Mutex<Sink>>,
    duration: Arc<Mutex<Duration>>,
    progress: Arc<Mutex<Duration>>,
    eq_settings: Arc<Mutex<EQSettings>>,
    is_playing: Arc<AtomicBool>,
    last_update: Arc<Mutex<Instant>>,
    looping: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    volume: Arc<Mutex<f32>>,
    queue: Arc<Mutex<Queue>>,
    lossless: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub fn setup() -> (Self, rodio::OutputStream) {
        let (stream, stream_handle) = rodio::OutputStream::try_default().expect("Failed to get default output device");
        let sink = Sink::try_new(&stream_handle).expect("Failed to create sink");
        let duration = Duration::from_secs(0);

        (
            Self {
                stream_handle,
                sink: Arc::new(Mutex::new(sink)),
                duration: Arc::new(Mutex::new(duration)),
                progress: Arc::new(Mutex::new(Duration::from_secs(0))),
                eq_settings: Arc::new(Mutex::new(EQSettings {
                    values: std::collections::HashMap::new(),
                })),
                is_playing: Arc::new(AtomicBool::new(false)),
                last_update: Arc::new(Mutex::new(Instant::now())),
                looping: Arc::new(AtomicBool::new(false)),
                muted: Arc::new(AtomicBool::new(false)),
                volume: Arc::new(Mutex::new(1.0)),
                queue: Arc::new(Mutex::new(Queue::new())),
                lossless: Arc::new(AtomicBool::new(false)),
            },
            stream,
        )
    }

    fn get_playback_position(&self) -> Duration {
        let mut progress = self.progress.lock().unwrap();
        let mut last_update = self.last_update.lock().unwrap();

        if self.is_playing.load(Ordering::Relaxed) {
            let now = Instant::now();
            let elapsed = now.duration_since(*last_update);
            *progress += elapsed;
            *last_update = now;
        }

        *progress
    }

    fn play(&self) {
        let volume = *self.volume.lock().unwrap();
        println!("Current volume: {}", volume);
        self.sink.lock().unwrap().play();
        self.is_playing.store(true, Ordering::Relaxed);
        *self.last_update.lock().unwrap() = Instant::now();
    }

    fn pause(&self) {
        self.sink.lock().unwrap().pause();
        self.is_playing.store(false, Ordering::Relaxed);
        self.get_playback_position();
    }

    fn set_looping(&self, looping: bool) {
        self.looping.store(looping, Ordering::Relaxed);
    }

    fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
        self.sink
            .lock()
            .unwrap()
            .set_volume(if muted { 0.0 } else { *self.volume.lock().unwrap() });
    }

    fn set_volume(&self, volume: f32) {
        *self.volume.lock().unwrap() = volume;
        if !self.muted.load(Ordering::Relaxed) {
            self.sink.lock().unwrap().set_volume(volume);
        }
    }

    fn set_eq_settings(&self, settings: EQSettings) {
        *self.eq_settings.lock().unwrap() = settings;
    }

    fn skip(&self) {
        let mut queue = self.queue.lock().unwrap();
        if let Some(song) = queue.next() {
            if let Ok(file) = self.load_song_file(&song) {
                self.load_song(song.clone(), file);
            }
        }
    }

    fn skip_to(&self, percentage: f32) {
        let duration = self.duration.lock().unwrap();
        let position = (duration.as_secs_f32() * percentage) as u64;
        self.seek(Duration::from_secs(position));
    }

    fn seek(&self, position: Duration) {
        let sink = self.sink.lock().unwrap();
        let was_playing = self.is_playing.load(Ordering::Relaxed);

        if was_playing {
            sink.pause();
            self.is_playing.store(false, Ordering::Relaxed);
        }

        *self.progress.lock().unwrap() = position;
        *self.last_update.lock().unwrap() = Instant::now();

        if was_playing {
            sink.play();
            self.is_playing.store(true, Ordering::Relaxed);
        }
    }

    fn load_song_file(&self, song: &Song) -> Result<BufReader<File>, String> {
        let mut song_path = crate::utils::commands::get_music_path();
        song_path.push("Songs");

        let flac_name = format!("{}.flac", song.id);
        let mp3_name = format!("{}.mp3", song.id);

        song_path.push(&flac_name);
        if song_path.exists() {
            return File::open(song_path)
                .map(BufReader::new)
                .map_err(|e| e.to_string());
        }

        song_path.pop();
        song_path.push(&mp3_name);
        if song_path.exists() {
            return File::open(song_path)
                .map(BufReader::new)
                .map_err(|e| e.to_string());
        }

        Err(format!("Song file not found: neither {} nor {}", flac_name, mp3_name))
    }

    fn load_song(&self, song: Song, file: BufReader<File>) {
        let sink = self.sink.lock().unwrap();
        let was_playing = self.is_playing.load(Ordering::Relaxed);

        sink.stop();

        let decoder = Decoder::new(file).unwrap().convert_samples::<f32>();
        let db_gains = vec![4.6, 8.0, 4.6, 0.9, 0.0, 3.0, 0.9, 0.0, 0.0, 0.0];
        let source = Equalizer::new(decoder, db_gains);

        sink.append(source);
        *self.duration.lock().unwrap() = Duration::from_millis(song.duration.try_into().unwrap());
        *self.progress.lock().unwrap() = Duration::from_secs(0);
        *self.last_update.lock().unwrap() = Instant::now();

        if was_playing {
            sink.play();
            self.is_playing.store(true, Ordering::Relaxed);
        }
    }
}

struct Equalizer<S> where S: Source<Item = f32> {
    source: S,
    filters: Vec<BiquadFilter>,
}

struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    fn new(frequency: f32, q: f32, gain: f32, sample_rate: u32) -> Self {
        let omega = (2.0 * PI * frequency) / (sample_rate as f32);
        let alpha = omega.sin() / (2.0 * q);
        let a = (10.0f32).powf(gain / 40.0);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * omega.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * omega.cos();
        let a2 = 1.0 - alpha / a;

        BiquadFilter {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output =
            self.b0 * input +
            self.b1 * self.x1 +
            self.b2 * self.x2 -
            self.a1 * self.y1 -
            self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

impl<S> Equalizer<S> where S: Source<Item = f32> {
    fn new(source: S, gains: Vec<f32>) -> Self {
        let sample_rate = source.sample_rate();
        let frequencies = [
            32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
        ];
        let filters = frequencies
            .iter()
            .zip(gains.iter())
            .map(|(&freq, &gain)| BiquadFilter::new(freq, 1.41, gain, sample_rate))
            .collect();

        Equalizer { source, filters }
    }
}

impl<S> Iterator for Equalizer<S> where S: Source<Item = f32> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.source
            .next()
            .map(|sample| { self.filters.iter_mut().fold(sample, |s, filter| filter.process(s)) })
    }
}

impl<S> Source for Equalizer<S> where S: Source<Item = f32> {
    fn current_frame_len(&self) -> Option<usize> {
        self.source.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

#[tauri::command]
pub fn set_looping(audio_player: tauri::State<AudioPlayer>, looping: bool) {
    audio_player.set_looping(looping);
}

#[tauri::command]
pub fn set_muted(audio_player: tauri::State<AudioPlayer>, muted: bool) {
    audio_player.set_muted(muted);
}

#[tauri::command]
pub fn set_volume(audio_player: tauri::State<AudioPlayer>, volume: f32) {
    audio_player.set_volume(volume);
}

#[tauri::command]
pub fn set_eq_settings(audio_player: tauri::State<AudioPlayer>, settings: EQSettings) {
    audio_player.set_eq_settings(settings);
}

#[tauri::command]
pub fn skip(audio_player: tauri::State<AudioPlayer>) {
    audio_player.skip();
}

#[tauri::command]
pub fn skip_to(audio_player: tauri::State<AudioPlayer>, percentage: f32) {
    audio_player.skip_to(percentage);
}

#[tauri::command]
pub fn seek(audio_player: tauri::State<AudioPlayer>, position: u64) {
    audio_player.seek(Duration::from_secs(position));
}

#[tauri::command]
pub async fn load_song(
    audio_player: tauri::State<'_, AudioPlayer>,
    song: Song
) -> Result<(), String> {
    let file = audio_player.load_song_file(&song)?;
    audio_player.load_song(song, file);
    Ok(())
}

#[tauri::command]
pub fn play(state: tauri::State<AudioPlayer>) {
    state.play();
}

#[tauri::command]
pub fn pause(state: tauri::State<AudioPlayer>) {
    state.pause();
}

#[tauri::command]
pub fn play_pause(state: tauri::State<AudioPlayer>) {
    if state.is_playing.load(Ordering::Relaxed) {
        state.pause();
    } else {
        state.play();
    }
}

#[tauri::command]
pub fn rewind(audio_player: tauri::State<AudioPlayer>) {
    audio_player.seek(Duration::from_secs(0));
}

unsafe impl Send for AudioPlayer {}
unsafe impl Sync for AudioPlayer {}
