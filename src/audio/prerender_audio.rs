use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, StreamConfig};
use xsynth_core::channel::{ChannelAudioEvent, ChannelConfigEvent, ChannelEvent, ChannelInitOptions, ControlEvent, VoiceChannel};
use xsynth_core::channel_group::{ChannelGroup, ChannelGroupConfig, ParallelismOptions, SynthEvent, SynthFormat};
use xsynth_core::soundfont::{SampleSoundfont, SoundfontBase};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use xsynth_core::{channel, channel_group, soundfont, AudioPipe, AudioStreamParams, ChannelCount};
use std::cell::UnsafeCell;

use crate::midi::midi_track_parser::{MIDIEvent, MIDIEventType};
use crate::util::global_timer::GlobalTimer;

pub struct UnsafeVec<T> {
    data: UnsafeCell<Vec<T>>
}

impl<T> UnsafeVec<T> {
    pub fn new(vec: Vec<T>) -> Self {
        UnsafeVec { data: UnsafeCell::new(vec) }
    }

    pub fn push(&mut self, arg: T) {
        self.data.get_mut().push(arg);
    }

    pub unsafe fn index_mut(&self, index: usize) -> &mut T {
        &mut (*self.data.get())[index]
    }

    pub unsafe fn slice_mut(&self, start: usize, end: usize) -> &mut [T] {
        &mut (*self.data.get())[start..end]
    }

    pub unsafe fn len(&self) -> usize {
        (*self.data.get()).len()
    }
}

unsafe impl<T> Sync for UnsafeVec<T> {}
unsafe impl<T> Send for UnsafeVec<T> {}

pub struct Limiter {
    loudness_l: f32,
    loudness_r: f32,
    velocity_r: f32,
    velocity_l: f32,
    pub attack: f32,
    pub falloff: f32,
    strength: f32,
    min_thresh: f32,
}

impl Limiter {
    pub fn new(attack: f32, release: f32, sample_rate: f32) -> Self {
        Self {
            loudness_l: 1.0,
            loudness_r: 1.0,
            velocity_l: 0.0,
            velocity_r: 0.0,
            attack: attack * sample_rate,
            falloff: release * sample_rate,
            strength: 1.0,
            min_thresh: 0.4,
        }
    }

    pub fn apply_limiter(&mut self, buffer: &mut [f32]) -> () {
        let count = buffer.len();
        for i in (0..count).step_by(2) {
            let mut l = buffer[i].abs();
            let mut r = buffer[i+1].abs();

            if self.loudness_l > l {
                self.loudness_l = (self.loudness_l * self.falloff + l) / (self.falloff + 1.0);
            } else {
                self.loudness_l = (self.loudness_l * self.attack + l) / (self.attack + 1.0);
            }

            if self.loudness_r > r {
                self.loudness_r = (self.loudness_r * self.falloff + r) / (self.falloff + 1.0);
            } else {
                self.loudness_r = (self.loudness_r * self.attack + r) / (self.attack + 1.0);
            }

            if self.loudness_l < self.min_thresh { self.loudness_l = self.min_thresh; }
            if self.loudness_r < self.min_thresh { self.loudness_r = self.min_thresh; }

            l = buffer[i] / (self.loudness_l * self.strength + 2.0 * (1.0 - self.strength)) / 2.0;
            r = buffer[i + 1] / (self.loudness_r * self.strength + 2.0 * (1.0 - self.strength)) / 2.0;

            if i != 0 {
                let dl = (buffer[i] - l).abs();
                let dr = (buffer[i+1] - r).abs();

                if self.velocity_l > dl {
                    self.velocity_l = (self.velocity_l * self.falloff + dl) / (self.falloff + 1.0);
                } else {
                    self.velocity_l = (self.velocity_l * self.attack + dl) / (self.attack + 1.0);
                }

                if self.velocity_r > dr {
                    self.velocity_r = (self.velocity_r * self.falloff + dr) / (self.falloff + 1.0);
                } else {
                    self.velocity_r = (self.velocity_r * self.attack + dr) / (self.attack + 1.0);
                }
            }

            buffer[i] = l;
            buffer[i+1] = r;
        }
    }
}

pub struct PrerenderAudio {
    read_pos: Arc<Mutex<usize>>,
    write_pos: Arc<Mutex<usize>>,

    audio_buffer: Arc<UnsafeVec<f32>>,
    pub device: Device,
    pub cfg: StreamConfig,

    midi_evs: Arc<Mutex<Vec<MIDIEvent>>>,
    g_time: Arc<Mutex<GlobalTimer>>,

    pub reset_requested: Arc<Mutex<bool>>,
    pub sample_rate: f32,

    stream_params: AudioStreamParams,
    xsynth_pre: Arc<Mutex<ChannelGroup>>,

    pub limiter: Arc<Mutex<Limiter>>,
    generator_thread: Option<std::thread::JoinHandle<()>>,
    start_time: f32,

    // audio settings
    pub audio_fps: f32
}

impl PrerenderAudio {
    pub fn new(buffer_length_secs: f32, global_time: Arc<Mutex<GlobalTimer>>) -> Self {
        // init audio
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let cfg = device.default_output_config().unwrap();
        let mut cfg: StreamConfig = cfg.into();
        cfg.buffer_size = BufferSize::Fixed(1024);

        let sr = cfg.sample_rate.0;

        let stream_params = AudioStreamParams::new(cfg.sample_rate.0, ChannelCount::Stereo);

        let s = Self {
            read_pos: Arc::new(Mutex::new(0)),
            write_pos: Arc::new(Mutex::new(0)),
            audio_buffer: Arc::new(UnsafeVec::new(vec![0.0; (buffer_length_secs * cfg.sample_rate.0 as f32) as usize * 2])),
            device,
            cfg,
            midi_evs: Arc::new(Mutex::new(Vec::new())),

            g_time: global_time,

            reset_requested: Arc::new(Mutex::new(false)),
            sample_rate: sr as f32,

            stream_params,
            xsynth_pre: Arc::new(Mutex::new(ChannelGroup::new(
                ChannelGroupConfig {
                    channel_init_options: ChannelInitOptions {
                        fade_out_killing: true
                    },
                    format: SynthFormat::Midi,
                    audio_params: stream_params,
                    parallelism: ParallelismOptions {
                        channel: channel_group::ThreadCount::Auto,
                        key: channel_group::ThreadCount::None
                    }
                },
            ))),

            limiter: Arc::new(Mutex::new(Limiter::new(0.01, 0.1, sr as f32))),
            generator_thread: None,
            start_time: 0.0f32,

            audio_fps: 0.0f32,
        };

        s
    }

    pub fn reset(&mut self) {
        *self.read_pos.lock().unwrap() = 0;
        *self.write_pos.lock().unwrap() = 0;
        //*self.audio_buffer = UnsafeVec::new(vec![0.0; (buffer_length_secs * cfg.sample_rate.0 as f32) as usize * 2])
        self.set_midi_events(Vec::new());
        (*self.xsynth_pre.lock().unwrap()).send_event(SynthEvent::AllChannels(
            ChannelEvent::Audio(
                ChannelAudioEvent::AllNotesKilled
            )
        ));
    }

    pub fn get_buffer_seconds(&self) -> f32 {
        let mut secs = {
            0.0f32.max((*self.write_pos.lock().unwrap()) as f32 - (*self.read_pos.lock().unwrap()) as f32) / self.sample_rate
        };
        return secs;
    }

    pub fn get_player_time(&self) -> f32 {
        let read_pos = *self.read_pos.lock().unwrap();
        return self.start_time + read_pos as f32 / self.sample_rate;
    }

    pub fn xsynth_load_sfs(&mut self, sfs: &[String]) {
        println!("Loading soundfonts lists...");
        let mut synth_soundfonts: Vec<Arc<dyn SoundfontBase>> = Vec::new();
        for sf in sfs {
            println!("appended {}", sf);
            synth_soundfonts.push(Arc::new(
                SampleSoundfont::new(std::path::Path::new(sf), self.stream_params, Default::default()).unwrap()
            ));
        }

        (*self.xsynth_pre.lock().unwrap()).send_event(
            SynthEvent::AllChannels(
                ChannelEvent::Config(
                    ChannelConfigEvent::SetSoundfonts(
                        synth_soundfonts.clone(),
                    )
                )
            )
        );
    }

    pub fn xsynth_set_layer_count(&mut self, layer_count: usize) {
        (*self.xsynth_pre.lock().unwrap()).send_event(
            SynthEvent::AllChannels(
                ChannelEvent::Config(
                ChannelConfigEvent::SetLayerCount
                    (
                        Some(layer_count),
                    )
                )
            )
        );
    }

    pub fn set_midi_events(&mut self, evs: Vec<MIDIEvent>) {
        *self.midi_evs.lock().unwrap() = evs;
    }

    pub fn render_audio(&mut self, start_time: f32, speed: f32) -> std::thread::JoinHandle<()> {
        *self.reset_requested.lock().unwrap() = false;

        // wtf.
        let write_pos = self.write_pos.clone();
        let read_pos = self.read_pos.clone();
        let midi_evs = self.midi_evs.clone();
        let audio_buffer = self.audio_buffer.clone();
        let reset_requested = self.reset_requested.clone();
        let xsynth_pre = self.xsynth_pre.clone();
        let audio_fps = self.audio_fps;

        let sample_rate = self.sample_rate;

        let mut wrote: usize = 0;

        std::thread::spawn(move || {
            (*read_pos.lock().unwrap()) = 0;
            (*write_pos.lock().unwrap()) = 0;

            let mut read = 0;
            let mut needs_reset = false;
            {
                read = *read_pos.lock().unwrap();
            }

            unsafe {
                let get_skipping_velocity = |wr: usize, rd: usize| {
                    //if (*g_time.lock().unwrap()).paused { return 0u8; }
                    let mut diff = 127 + 10 - (wr as i32 - rd as i32) / 100;
                    if diff > 127 { diff = 127; }
                    if diff < 0 { diff = 0; }
                    diff as u8
                };

                for e in (*midi_evs.lock().unwrap()).iter_mut() {
                    {
                        read = *read_pos.lock().unwrap();
                        needs_reset = *reset_requested.lock().unwrap();
                    }

                    if match e.command {
                        MIDIEventType::NoteOn | MIDIEventType::NoteOff => true,
                        _ => false
                    } && e.time / speed < start_time {
                        continue;
                    }

                    if wrote < read {
                        wrote = read;
                    }

                    let ev_time = e.time / speed;

                    let offset = if audio_fps > 0.0 {
                        f32::floor(ev_time * audio_fps) / audio_fps - start_time
                    } else {
                        ev_time - start_time
                    };

                    let samples = (offset * sample_rate) as isize - wrote as isize;
                    if samples > 0 {
                        let mut samples = samples as usize;
                        while wrote + samples > read + audio_buffer.len() / 2 {
                            let mut spare = (read + audio_buffer.len() / 2) - wrote;
                            if spare > 0 {
                                if spare > samples { spare = samples; }
                                if spare != 0 {
                                    let start = (wrote * 2) % audio_buffer.len();
                                    let mut count = spare * 2;
                                    if start + count > audio_buffer.len() {
                                        (xsynth_pre.lock().unwrap()).read_samples(audio_buffer.slice_mut(start, audio_buffer.len()));
                                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, audio_buffer.len()));
                                        count -= audio_buffer.len() - start;
                                        (xsynth_pre.lock().unwrap()).read_samples(audio_buffer.slice_mut(0, count));
                                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(0, count));
                                    } else {
                                        (xsynth_pre.lock().unwrap()).read_samples(audio_buffer.slice_mut(start, (start + count)));
                                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, (start + count)));
                                    }
                                    samples -= spare;
                                    wrote += spare;
                                }
                                if samples == 0 { break; }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(2));
                            if needs_reset {
                                break;
                            }
                        }
                        if samples != 0 {
                            let start = (wrote * 2) % audio_buffer.len();
                            let mut count = samples * 2;
                            if start + count > audio_buffer.len() {
                                (xsynth_pre.lock().unwrap()).read_samples(audio_buffer.slice_mut(start, audio_buffer.len()));
                                //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, audio_buffer.len()));
                                count -= audio_buffer.len() - start;
                                (xsynth_pre.lock().unwrap()).read_samples(audio_buffer.slice_mut(0, count));
                                //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(0, count));
                            } else {
                                (*xsynth_pre.lock().unwrap()).read_samples(audio_buffer.slice_mut(start, (start + count)));
                                //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, (start + count)));
                            }
                        }
                        wrote += samples;
                    }

                    match e.command {
                        MIDIEventType::NoteOn => {
                            let key = e.data[1];
                            let vel = e.data[2];
                            if vel > get_skipping_velocity(wrote, read) {
                                (xsynth_pre.lock().unwrap()).send_event(
                                    SynthEvent::Channel(e.data[0] as u32, 
                                        ChannelEvent::Audio(ChannelAudioEvent::NoteOn {
                                            key,
                                            vel
                                        })
                                    )
                                );
                            }
                        },
                        MIDIEventType::NoteOff => {
                            let key = e.data[1];
                            (xsynth_pre.lock().unwrap()).send_event(
                                SynthEvent::Channel(e.data[0] as u32, 
                                    ChannelEvent::Audio(ChannelAudioEvent::NoteOff {
                                        key
                                    }
                                )
                            ));
                        },
                        MIDIEventType::ControlEvent => {
                            let num = e.data[1];
                            let val = e.data[2];
                            (xsynth_pre.lock().unwrap()).send_event(
                                SynthEvent::Channel(e.data[0] as u32, 
                                    ChannelEvent::Audio(ChannelAudioEvent::Control(
                                        ControlEvent::Raw(num, val)
                                    )
                                )
                            ));
                        },
                        _ => {

                        }
                    }

                    *write_pos.lock().unwrap() = wrote;
                    if needs_reset {
                        (xsynth_pre.lock().unwrap()).send_event(SynthEvent::AllChannels(
                            ChannelEvent::Audio(
                                ChannelAudioEvent::AllNotesKilled
                            )
                        ));
                        break;
                    }
                }
            }
        })
    }

    fn kill_last_generator(&mut self) -> () {
        unsafe {
            for i in 0..self.audio_buffer.len() {
                *self.audio_buffer.index_mut(i) = 0.0;
            }
            if *self.reset_requested.lock().unwrap() == false {
                *self.reset_requested.lock().unwrap() = true;
            }
            if self.generator_thread.is_some() {
                self.generator_thread
                    .take().expect("trying to stop generator thread, which isn't running")
                    .join().unwrap();
            }
        }
    }

    pub fn start(&mut self, start_time: f32, speed: f32) -> () {
        self.kill_last_generator();
        *self.reset_requested.lock().unwrap() = false;
        self.start_time = start_time / speed;
        self.generator_thread = Some(self.render_audio(self.start_time, speed));
    }

    pub fn stop(&mut self) -> () {
        self.kill_last_generator();
        *self.reset_requested.lock().unwrap() = false;
        self.generator_thread = None;
        *self.read_pos.lock().unwrap() = 0;
        *self.write_pos.lock().unwrap() = 0;
    }

    pub fn sync_player(&mut self, time: f32, speed: f32) -> () {
        unsafe {
            let mut read_pos = self.read_pos.lock().unwrap();
            let time = time / speed;
            let t = self.start_time + (*read_pos as f32) / self.sample_rate;
            let offs = time - t;
            let mut new_pos = *read_pos as i32 + (offs * self.sample_rate) as i32;
            if new_pos < 0 {
                new_pos = 0;
            }
            if (*read_pos as i32 - new_pos).abs() as f32 / self.sample_rate > 0.03 {
                *read_pos = new_pos as usize;
            }
        }
    }

    pub fn construct_stream(&mut self) -> cpal::Stream {
        let g_time = self.g_time.clone();
        let read_pos = self.read_pos.clone();
        let write_pos = self.write_pos.clone();
        let audio_buffer = self.audio_buffer.clone();
        let limiter = self.limiter.clone();

        let stream = self.device.build_output_stream(&self.cfg, move |data: &mut [f32], _| {
            let count = data.len();
            unsafe {
                if (*g_time.lock().unwrap()).paused {
                    for i in 0..count {
                        data[i] = 0.0;
                    }
                    return;
                }
                let read = *read_pos.lock().unwrap() % (audio_buffer.len() / 2);
                if *read_pos.lock().unwrap() + count / 2 > * write_pos.lock().unwrap()  {
                    for i in 0..count {
                        data[i] = 0.0;
                    }
                    //println!("!! buffer is behind by {} secs !!", ((*arc_read_clone.lock().unwrap() + count / 2) as i32 - *arc_write_clone.lock().unwrap() as i32) as f32 / ssr)
                } else {
                    for i in 0..count {
                        data[i] = *audio_buffer.index_mut((i + read * 2) % (audio_buffer.len()));
                    }
                }

                *read_pos.lock().unwrap() += data.len() / 2;
                limiter.lock().unwrap().apply_limiter(data);
            }
        }, |err| {
            println!("{}",err.to_string());
        }, None).unwrap();
        stream
    }

    pub fn play_audio(&mut self, time: f32, speed: f32, mut force: bool) -> () {
        //let mut g_time = self.g_time.clone();
        
        if !force {
            let time = time;
            if time + 0.1 > self.get_player_time() + self.get_buffer_seconds() || time + 0.01 < self.get_player_time() {
                force = true;
            }
        }

        if force {
            self.start(time, speed);
        } else {
            self.sync_player(time, speed);
        }
    }
}