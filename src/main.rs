mod midi;
mod util;
mod wavout;
mod rendering;
mod settings;
mod audio;

use std::cell::UnsafeCell;
use std::path::Path;
use std::thread::{self, sleep};

use audio::prerender_audio::PrerenderAudio;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BufferSize, StreamConfig};
use midi::midi_track_parser::{MIDIEvent, MIDIEventType, Note};
use midi::midi_file::MIDIFile;
use rendering::window::MainWindow;
use util::global_timer::GlobalTimer;
use xsynth_core::channel::{ChannelAudioEvent, ChannelConfigEvent, ChannelEvent, ChannelInitOptions, ControlEvent, VoiceChannel};
use xsynth_core::channel_group::{ChannelGroup, ChannelGroupConfig, ParallelismOptions, SynthEvent, SynthFormat};
use xsynth_core::soundfont::{SampleSoundfont, SoundfontBase};
use std::sync::{Arc, Mutex};

use xsynth_core::{channel, channel_group, soundfont, AudioPipe, AudioStreamParams, ChannelCount};

#[derive(PartialEq, Eq)]
pub enum PlayerState {
    Playing=0,
    Paused=1
}

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

fn main() {
    // play state
    let glob_timer: Arc<Mutex<GlobalTimer>> = Arc::new(Mutex::new(GlobalTimer::new()));
    glob_timer.lock().unwrap().pause();

    /*let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let cfg = device.default_output_config().unwrap();
    let mut cfg: StreamConfig = cfg.into();
    cfg.buffer_size = BufferSize::Fixed(1024);

    let stream_params = AudioStreamParams::new(44100, ChannelCount::Stereo);

    println!("intializing sf list");
    let synth_soundfonts: Vec<Arc<dyn SoundfontBase>> = vec![Arc::new(
        SampleSoundfont::new(Path::new("./test/Kryo Keys II (SF2).sf2"), stream_params, Default::default()).unwrap()
    )];

    println!("loading sf...");

    //let threadpool = rayon::ThreadPoolBuilder::new().build().unwrap();

    let mut new_ch = {
        let mut ch = /*VoiceChannel::new(
            Default::default(), 
            stream_params,
            Some(Arc::new(threadpool))
        );*/
        ChannelGroup::new(
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
            }
        );
        ch.send_event(
            SynthEvent::AllChannels(
                ChannelEvent::Config(
                    ChannelConfigEvent::SetSoundfonts(
                        synth_soundfonts.clone(),
                    )
                )
            )
        );
        ch.send_event(
            SynthEvent::AllChannels(
                ChannelEvent::Config(
                ChannelConfigEvent::SetLayerCount
                    (
                        Some(4),
                    )
                )
            )
        );
        ch
    };


    println!("done");

    let mut wrote: usize = 0;

    let buffer_secs: f32 = 60.0;
    let buffer_length = (stream_params.sample_rate as f32 * buffer_secs) as usize;

    //let m: MIDIFile = MIDIFile::new(String::from("./test/tau2.5.9.mid")).unwrap();
    // im dumb so idk how to resolve this without rust compiler yelling at me about lifetimes
    //let (mut evs, notes): (Vec<MIDIEvent>, Vec<Vec<Note>>) = m.get_evs_and_notes();

    let audio_buffer = Arc::new(UnsafeVec::<f32>::new(vec![0.0f32; buffer_length * 2]));
    let mut aud_arc = Arc::clone(&audio_buffer);
    //let mut buffer_locked = Arc::new(Mutex::new(false));
    //let mut buf_locked_clone = buffer_locked.clone();

    let ssr = stream_params.sample_rate as f32;

    let mut lim = Limiter::new(0.001, 0.1, stream_params.sample_rate as f32);
    let start_time = 5.0 * 60.0;

    let arc_read= Arc::new(Mutex::new(0));
    let arc_read_clone = arc_read.clone();
    let arc_write= Arc::new(Mutex::new(0));
    let arc_write_clone = arc_write.clone();

    let g_time = glob_timer.clone();

    /*let _ = thread::spawn(move || {

        let aud_buf = &mut aud_arc;
        {
            *arc_read.lock().unwrap() = 0;
        }

        unsafe {
            let get_skipping_velocity = |wr: usize| {
                if (*g_time.lock().unwrap()).paused { return 0u8; }
                let mut diff = 127 + 10 - (wr as i32 - *arc_read.lock().unwrap() as i32) / 100;
                if diff > 127 { diff = 127; }
                if diff < 0 { diff = 0; }
                diff as u8
            };

            for e in evs[..].iter_mut() {
                if match e.command {
                    MIDIEventType::NoteOn | MIDIEventType::NoteOff => true,
                    _ => false
                } && e.time < start_time {
                    continue;
                }

                if wrote < *arc_read.lock().unwrap() {
                    wrote = *arc_read.lock().unwrap();
                }
                let offset = e.time - start_time;
                let samples = (offset * ssr) as isize - wrote as isize;
                if samples > 0 {
                    let mut samples = samples as usize;
                    while wrote + samples > *arc_read.lock().unwrap() + buffer_length {
                        let mut spare = (*arc_read.lock().unwrap() + buffer_length) - wrote;
                        if spare > 0 {
                            if spare > samples { spare = samples; }
                            if spare != 0 {
                                let start = (wrote) % (buffer_length);
                                let mut count = spare;
                                if start + count > buffer_length {
                                    new_ch.read_samples(aud_buf.slice_mut(start * 2, buffer_length * 2));
                                    lim.apply_limiter(aud_buf.slice_mut(start * 2, buffer_length * 2));
                                    count -= buffer_length - start;
                                    new_ch.read_samples(aud_buf.slice_mut(0, count * 2));
                                    lim.apply_limiter(aud_buf.slice_mut(0, count * 2));
                                } else {
                                    new_ch.read_samples(aud_buf.slice_mut(start * 2, (start + count) * 2));
                                    lim.apply_limiter(aud_buf.slice_mut(start * 2, (start + count) * 2));
                                }
                                samples -= spare;
                                wrote += spare;
                            }
                            if samples == 0 { break; }
                        }
                        //std::thread::sleep(Duration::from_millis(2));
                    }
                    if samples != 0 {
                        let start = (wrote) % (buffer_length);
                        let mut count = samples;
                        if start + count > buffer_length {
                            new_ch.read_samples(aud_buf.slice_mut(start * 2, buffer_length * 2));
                            lim.apply_limiter(aud_buf.slice_mut(start * 2, buffer_length * 2));
                            count -= buffer_length - start;
                            new_ch.read_samples(aud_buf.slice_mut(0, count * 2));
                            lim.apply_limiter(aud_buf.slice_mut(0, count * 2));
                        } else {
                            new_ch.read_samples(aud_buf.slice_mut(start * 2, (start + count) * 2));
                            lim.apply_limiter(aud_buf.slice_mut(start * 2, (start + count) * 2));
                        }
                    }
                    wrote += samples;
                }
                
                match e.command {
                    MIDIEventType::NoteOn => {
                        let key = e.data[1];
                        let vel = e.data[2];
                        if vel > 20 {
                            new_ch.send_event(
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
                        new_ch.send_event(
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
                        new_ch.send_event(
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
                *arc_write.lock().unwrap() = wrote;
            }
        }
    });*/

    let mut audio_buffer = Arc::clone(&audio_buffer);

    let g_time = glob_timer.clone();*/

    //let stream = prerenderer.device.build_output_stream(&cfg, move |data: &mut [f32], _| {
        /*let audio_buf = &mut audio_buffer;
        let count = data.len();
        unsafe {
            if (*g_time.lock().unwrap()).paused {
                for i in 0..count {
                    data[i] = 0.0;
                }
                return;
            }
            let read = *arc_read_clone.lock().unwrap() % buffer_length;
            if *arc_read_clone.lock().unwrap() + count / 2 > *arc_write_clone.lock().unwrap()  {
                for i in 0..count {
                    data[i] = 0.0;
                }
                //println!("!! buffer is behind by {} secs !!", ((*arc_read_clone.lock().unwrap() + count / 2) as i32 - *arc_write_clone.lock().unwrap() as i32) as f32 / ssr)
            } else {
                for i in 0..count {
                    data[i] = *audio_buf.index_mut((i + read * 2) % (buffer_length * 2)) * 0.5;
                }
            }
            *arc_read_clone.lock().unwrap() += data.len() / 2;
        }*/
    //}, |err| {()}, None).expect("your stream failed");

    let _ = MainWindow::new(1280, 720, "KansoMIDI", glob_timer.clone());
}