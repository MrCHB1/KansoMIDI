use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BufferSize, Device, StreamConfig};
use xsynth_core::channel::{ChannelAudioEvent, ChannelConfigEvent, ChannelEvent, ChannelInitOptions, ControlEvent};
use xsynth_core::channel_group::{ChannelGroup, ChannelGroupConfig, ParallelismOptions, SynthEvent, SynthFormat};
use xsynth_core::soundfont::{EnvelopeCurveType, EnvelopeOptions, Interpolator, SampleSoundfont, SoundfontBase, SoundfontInitOptions};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, atomic::AtomicUsize};
use xsynth_core::{channel_group, AudioPipe, AudioStreamParams, ChannelCount};
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

    /*pub fn push(&mut self, arg: T) {
        self.data.get_mut().push(arg);
    }*/

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

    /// applies a filter to prevent audio clipping above 1 dB. 
    /// * `buffer` - the slice of the samples to apply the filter to
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
    read_pos: Arc<AtomicUsize>,
    write_pos: Arc<AtomicUsize>,

    audio_buffer: Arc<UnsafeVec<f32>>,
    pub device: Device,
    pub cfg: StreamConfig,

    midi_evs: Arc<Mutex<Vec<MIDIEvent>>>,
    g_time: Arc<Mutex<GlobalTimer>>,

    pub reset_requested: Arc<AtomicBool>,
    pub sample_rate: f32,

    stream_params: AudioStreamParams,
    xsynth_pre: Arc<Mutex<ChannelGroup>>,

    pub limiter: Arc<Mutex<Limiter>>,
    generator_thread: Option<std::thread::JoinHandle<()>>,
    start_time: f32,

    // audio settings
    pub audio_fps: f32,
    pub transpose: i32,
}

impl PrerenderAudio {
    pub fn new(buffer_length_secs: f32, global_time: Arc<Mutex<GlobalTimer>>, key_threads: usize, channel_threads: usize) -> Self {
        // init audio
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let cfg = device.default_output_config().unwrap();
        let mut cfg: StreamConfig = cfg.into();
        cfg.buffer_size = BufferSize::Fixed(2048);

        let sr = cfg.sample_rate.0;

        let stream_params = AudioStreamParams::new(cfg.sample_rate.0, ChannelCount::Stereo);

        let s = Self {
            read_pos: Arc::new(AtomicUsize::new(0)),
            write_pos: Arc::new(AtomicUsize::new(0)),
            audio_buffer: Arc::new(UnsafeVec::new(vec![0.0; (buffer_length_secs * cfg.sample_rate.0 as f32) as usize * 2])),
            device,
            cfg,
            midi_evs: Arc::new(Mutex::new(Vec::new())),

            g_time: global_time,

            reset_requested: Arc::new(AtomicBool::new(false)),
            sample_rate: sr as f32,

            stream_params,
            xsynth_pre: Arc::new(Mutex::new(ChannelGroup::new(
                ChannelGroupConfig {
                    channel_init_options: ChannelInitOptions {
                        fade_out_killing: false
                    },
                    format: SynthFormat::Midi,
                    audio_params: stream_params,
                    parallelism: ParallelismOptions {
                        channel: match channel_threads {
                            0 => channel_group::ThreadCount::Auto,
                            1 => channel_group::ThreadCount::None,
                            _ => channel_group::ThreadCount::Manual(channel_threads)
                        },
                        key: match key_threads {
                            0 => channel_group::ThreadCount::Auto,
                            1 => channel_group::ThreadCount::None,
                            _ => channel_group::ThreadCount::Manual(key_threads)
                        }
                    }
                },
            ))),

            limiter: Arc::new(Mutex::new(Limiter::new(0.01, 0.1, sr as f32))),
            generator_thread: None,
            start_time: 0.0f32,

            audio_fps: 0.0f32,
            transpose: 0
        };

        s
    }

    pub fn get_buffer_seconds(&self) -> f32 {
        let secs = {
            let read_pos = self.read_pos.load(Ordering::Acquire);
            let write_pos = self.write_pos.load(Ordering::Acquire);
            0.0f32.max((write_pos) as f32 - (read_pos) as f32) / self.sample_rate
        };
        return secs;
    }

    pub fn get_player_time(&self) -> f32 {
        let read_pos = self.read_pos.load(Ordering::Acquire);
        return self.start_time + (read_pos) as f32 / self.sample_rate;
    }

    pub fn xsynth_load_sfs(&mut self, sfs: &[String]) {
        let mut synth_soundfonts: Vec<Arc<dyn SoundfontBase>> = Vec::new();
        for sf in sfs {
            println!("appended {}", sf);
            synth_soundfonts.push(Arc::new(
                SampleSoundfont::new(std::path::Path::new(sf), self.stream_params, SoundfontInitOptions {
                    bank: None,
                    preset: None,
                    vol_envelope_options: EnvelopeOptions {
                        attack_curve: EnvelopeCurveType::Linear,
                        decay_curve: EnvelopeCurveType::Linear,
                        release_curve: EnvelopeCurveType::Linear,
                    },
                    use_effects: true,
                    interpolator: Interpolator::Linear
                }).unwrap()
            ));
        }

        println!("attempting to load soundfonts...");

        (*self.xsynth_pre.lock().unwrap()).send_event(
            SynthEvent::AllChannels(
                ChannelEvent::Config(
                    ChannelConfigEvent::SetSoundfonts(
                        synth_soundfonts.clone(),
                    )
                )
            )
        );

        println!("soundfonts loaded!!");
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
        // wtf.
        let write_pos = self.write_pos.clone();
        let read_pos = self.read_pos.clone();
        let midi_evs = self.midi_evs.clone();
        let audio_buffer = self.audio_buffer.clone();
        let reset_requested = self.reset_requested.clone();
        let xsynth_pre = self.xsynth_pre.clone();
        let transpose = self.transpose;
        let audio_fps = self.audio_fps;

        let sample_rate = self.sample_rate;
        
        reset_requested.store(false, Ordering::Release);

        std::thread::spawn(move || {
            unsafe {
                let buff_len = audio_buffer.len();

                let get_skipping_velocity = |wr: usize, rd: usize| {
                    //if (*g_time.lock().unwrap()).paused { return 0u8; }
                    let mut diff = 127 + 10 - (wr as i32 - rd as i32) / 100;
                    if diff > 127 { diff = 127; }
                    if diff < 0 { diff = 0; }
                    diff as u8
                };

                let mut write_wrapped = |xsynth: &mut ChannelGroup, start: usize, count: usize| {
                    let start = (start * 2) % buff_len;
                    let mut count = count * 2;
                    if start + count > buff_len {
                        (xsynth).read_samples(audio_buffer.slice_mut(start, buff_len));
                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, audio_buffer.len()));
                        count -= buff_len - start;
                        (xsynth).read_samples(audio_buffer.slice_mut(0, count));
                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(0, count));
                    } else {
                        (xsynth).read_samples(audio_buffer.slice_mut(start, start + count));
                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, (start + count)));
                    }
                };
                /*(*read_pos.lock().unwrap()) = 0;
                (*write_pos.lock().unwrap()) = 0;

                let mut read = 0;
                let mut needs_reset = false;
                {
                    read = *read_pos.lock().unwrap();
                }*/
                read_pos.store(0, Ordering::Relaxed);
                write_pos.store(0, Ordering::Relaxed);
                let mut xsynth = xsynth_pre.lock().unwrap();

                //let needs_reset = reset_requested.lock().unwrap();
                //let mut write = 0;

                for e in (midi_evs.lock().unwrap()).iter() {
                    if match e.command {
                        MIDIEventType::NoteOn | MIDIEventType::NoteOff => true,
                        _ => false
                    } && (e.time / speed < start_time
                        //|| e.data[2] < 15
                        //|| e.data[2] < get_skipping_velocity(write_pos.load(Ordering::Relaxed), read_pos.load(Ordering::Relaxed))
                    ) {
                        continue;
                    }

                    if write_pos.load(Ordering::Relaxed) < read_pos.load(Ordering::Relaxed) {
                        write_pos.store(read_pos.load(Ordering::Relaxed), Ordering::Relaxed);
                    }

                    let ev_time = e.time / speed;

                    let offset = if audio_fps > 0.0 {
                        f32::floor(ev_time * audio_fps) / audio_fps - start_time
                    } else {
                        ev_time - start_time
                    };

                    let samples = (offset * sample_rate) as isize - write_pos.load(Ordering::Relaxed) as isize;
                    if samples > 0 {
                        let mut samples = samples as usize;
                        while write_pos.load(Ordering::Relaxed) + samples > read_pos.load(Ordering::Relaxed) + buff_len / 2 {
                            let mut spare = (read_pos.load(Ordering::Relaxed) + buff_len / 2) - write_pos.load(Ordering::Relaxed);
                            if spare > 0 {
                                if spare > samples { spare = samples; }
                                if spare != 0 {
                                    /*let start = (write_pos.load(Ordering::Acquire) * 2) % audio_buffer.len();
                                    let mut count = spare * 2;
                                    if start + count > audio_buffer.len() {
                                        (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(start, audio_buffer.len()));
                                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, audio_buffer.len()));
                                        count -= audio_buffer.len() - start;
                                        (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(0, count));
                                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(0, count));
                                    } else {
                                        (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(start, start + count));
                                        //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, (start + count)));
                                    }*/
                                    write_wrapped(&mut xsynth, write_pos.load(Ordering::Relaxed), spare);
                                    samples -= spare;
                                    write_pos.fetch_add(spare, Ordering::Relaxed);
                                    //*write_pos.lock().unwrap() = write;
                                }
                                if samples == 0 { break; }
                            }
                            //std::thread::sleep(std::time::Duration::from_millis(2));
                            if reset_requested.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                        if samples != 0 {
                            /*let start = (write_pos.load(Ordering::Acquire) * 2) % audio_buffer.len();
                            let mut count = samples * 2;
                            if start + count > audio_buffer.len() {
                                (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(start, audio_buffer.len()));
                                //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, audio_buffer.len()));
                                count -= audio_buffer.len() - start;
                                (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(0, count));
                                //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(0, count));
                            } else {
                                (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(start, start + count));
                                //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, (start + count)));
                            }*/
                            write_wrapped(&mut xsynth, write_pos.load(Ordering::Relaxed), samples);
                        }
                        write_pos.fetch_add(samples, Ordering::Relaxed);
                    }

                    match e.command {
                        MIDIEventType::NoteOn => {
                            let mut key = e.data[1];
                            if (key as i32) < -transpose { continue; }
                            key = (key as i32 + transpose) as u8;
                            
                            let vel = e.data[2];
                            if vel < get_skipping_velocity(write_pos.load(Ordering::Relaxed), read_pos.load(Ordering::Relaxed)) { continue; }
                            if vel < 15 { continue; }

                            (*xsynth).send_event(
                                SynthEvent::Channel(e.data[0] as u32, 
                                    ChannelEvent::Audio(ChannelAudioEvent::NoteOn {
                                        key,
                                        vel
                                    })
                                )
                            );
                        },
                        MIDIEventType::NoteOff => {
                            let mut key = e.data[1];
                            if (key as i32) < -transpose { continue; }
                            key = (key as i32 + transpose) as u8;

                            let vel = e.data[2];
                            if vel < get_skipping_velocity(write_pos.load(Ordering::Relaxed), read_pos.load(Ordering::Relaxed)) { continue; }
                            if vel < 15 { continue; }

                            (*xsynth).send_event(
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
                            (*xsynth).send_event(
                                SynthEvent::Channel(e.data[0] as u32, 
                                    ChannelEvent::Audio(ChannelAudioEvent::Control(
                                        ControlEvent::Raw(num, val)
                                    )
                                )
                            ));
                        },
                        MIDIEventType::PitchBend => {
                            let v1 = e.data[1];
                            let v2 = e.data[2];
                            let bend = (((v2 as i32) << 7) | v1 as i32) as f32 - 8192.0;
                            (*xsynth).send_event(
                                SynthEvent::Channel(e.data[0] as u32,
                                    ChannelEvent::Audio(ChannelAudioEvent::Control(
                                        ControlEvent::PitchBendValue(bend / 8192.0)
                                    )
                                )
                            ));
                        }
                        _ => {

                        }
                    }

                    if reset_requested.load(Ordering::Relaxed) {
                        break;
                    }
                }

                while !reset_requested.load(Ordering::Relaxed) {
                    let spare = (read_pos.load(Ordering::Relaxed) + buff_len / 2) - write_pos.load(Ordering::Relaxed);
                    if spare > 0 {
                        /*let start = (write_pos.load(Ordering::Acquire) * 2) % audio_buffer.len();
                        let mut count = spare * 2;
                        if start + count > audio_buffer.len() {
                            (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(start, audio_buffer.len()));
                            //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, audio_buffer.len()));
                            count -= audio_buffer.len() - start;
                            (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(0, count));
                            //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(0, count));
                        } else {
                            (*xsynth).read_samples_unchecked(audio_buffer.slice_mut(start, start + count));
                            //limiter.lock().unwrap().apply_limiter(audio_buffer.slice_mut(start, (start + count)));
                        }*/
                        write_wrapped(&mut xsynth, write_pos.load(Ordering::Relaxed), spare);
                        write_pos.fetch_add(spare, Ordering::Relaxed);
                    }
                }

                // reset at end of thread
                (*xsynth).send_event(SynthEvent::AllChannels(
                    ChannelEvent::Audio(
                        ChannelAudioEvent::AllNotesKilled
                    )
                ));
                /*(*xsynth).send_event(SynthEvent::AllChannels(
                    ChannelEvent::Audio(
                        ChannelAudioEvent::ResetControl
                    )
                ));*/
            }
        })
    }

    fn kill_last_generator(&mut self) -> () {
        unsafe {
            self.reset_requested.store(true, Ordering::Relaxed);
            if self.generator_thread.is_some() {
                self.generator_thread
                    .take().expect("trying to stop generator thread, which isn't running")
                    .join().unwrap();
            }
            for i in 0..self.audio_buffer.len() {
                *self.audio_buffer.index_mut(i) = 0.0;
            }
        }
    }

    pub fn start(&mut self, start_time: f32, speed: f32) -> () {
        self.kill_last_generator();
        self.start_time = start_time / speed;
        self.generator_thread = Some(self.render_audio(self.start_time, speed));
    }

    pub fn stop(&mut self) -> () {
        self.kill_last_generator();
        self.reset_requested.store(false, Ordering::Relaxed);
        self.generator_thread = None;
        self.read_pos.store(0, Ordering::Relaxed);
        self.write_pos.store(0, Ordering::Relaxed);
    }

    pub fn sync_player(&mut self, time: f32, speed: f32) -> () {
        let mut read_pos = self.read_pos.load(Ordering::Relaxed);
        let time = time / speed;
        let t = self.start_time + (read_pos as f32) / self.sample_rate;
        let offs = time - t;
        let mut new_pos = read_pos as i32 + (offs * self.sample_rate) as i32;
        if new_pos < 0 {
            new_pos = 0;
        }
        if (read_pos as i32 - new_pos).abs() as f32 / self.sample_rate > 0.03 {
            read_pos = new_pos as usize;
            self.read_pos.store(read_pos, Ordering::Relaxed);
        }
    }

    pub fn construct_stream(&mut self) -> cpal::Stream {
        let g_time = self.g_time.clone();
        let read_pos = self.read_pos.clone();
        let write_pos = self.write_pos.clone();
        let audio_buffer = self.audio_buffer.clone();
        let limiter: Arc<Mutex<Limiter>> = self.limiter.clone();
        let reset_requested = self.reset_requested.clone();

        let stream = self.device.build_output_stream(&self.cfg, move |data: &mut [f32], _| {
            let count = data.len();

            unsafe {
                if (*g_time.lock().unwrap()).paused || reset_requested.load(Ordering::Relaxed) {
                    for i in 0..count {
                        data[i] = 0.0;
                    }
                    return;
                }
                let read = read_pos.load(Ordering::Relaxed) % (audio_buffer.len() / 2);
                if read_pos.load(Ordering::Relaxed) + count / 2 > write_pos.load(Ordering::Relaxed)  {
                    let mut copy_count = read_pos.load(Ordering::Relaxed) - (write_pos.load(Ordering::Relaxed) + count / 2);
                    if copy_count > count / 2 { 
                        copy_count = count / 2;
                    }
                    if copy_count > 0 {
                        for i in 0..(copy_count * 2) {
                            data[i] = *audio_buffer.index_mut((i + read * 2) % (audio_buffer.len()));
                        }
                    } else {
                        copy_count = 0;
                    }
                    for i in (copy_count * 2)..count {
                        data[i] = 0.0;
                    }
                    //println!("!! buffer is behind by {} secs !!", ((*arc_read_clone.lock().unwrap() + count / 2) as i32 - *arc_write_clone.lock().unwrap() as i32) as f32 / ssr)
                } else {
                    for i in 0..count {
                        data[i] = *audio_buffer.index_mut((i + read * 2) % (audio_buffer.len()));
                    }
                }

                //*read_pos.lock().unwrap() += data.len() / 2;
                read_pos.fetch_add(data.len() / 2, Ordering::Relaxed);
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