use std::{error::Error, fmt, fs::File, io::BufReader, path::PathBuf, time::{Duration, Instant}};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, StreamConfig};
use gl;
use glutin::{
    dpi::{LogicalSize, PhysicalSize, Size},
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
    event_loop::{ControlFlow, EventLoop},
    ContextBuilder
};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use rand::seq::SliceRandom;
use crate::{
    audio::prerender_audio::PrerenderAudio, midi::{midi_file::MIDIFile, midi_track_parser::{MIDIEvent, Note}}, rendering::renderer::Renderer, settings::{audio_settings::AudioSettings, player_settings::PlayerSettings, visual_settings::VisualSettings}, util::{color_palettes::ColorPalettes, global_timer::GlobalTimer}, PlayerState
};
use std::sync::{Arc, Mutex};
use imgui::{Context, Ui};

use image::{GenericImageView, ImageReader, Rgb};
use rfd::FileDialog;

pub struct MainWindow {
    pub width: usize,
    pub height: usize,
    pub visual_settings: VisualSettings,
    pub audio_settings: AudioSettings,
    pub player_settings: PlayerSettings,
    title: &'static str,

    popup_ids: u16,
    midi_length: f32,
    prerenderer: PrerenderAudio,
    stream: Option<cpal::Stream>,

    color_palettes: ColorPalettes,

    midi_loaded: bool,
    stream_playing: bool,
    time_nav_changed: bool,
    sf_selected: i32,
    sf_loaded: bool,
}

impl MainWindow {
    pub fn new(width: usize, height: usize, title: &'static str, play_state: Arc<Mutex<GlobalTimer>>) -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let cfg = device.default_output_config().unwrap();
        let mut cfg: StreamConfig = cfg.into();
        cfg.buffer_size = BufferSize::Fixed(1024);

        let mut visual_settings = VisualSettings::new();
        let mut audio_settings = AudioSettings::new();
        let mut player_settings = PlayerSettings::new();

        visual_settings.load_settings();
        audio_settings.load_settings();
        player_settings.load_settings();

        let mut win = Self {
            width,
            height,
            title,

            visual_settings,
            audio_settings,
            player_settings,

            color_palettes: ColorPalettes::new(),

            popup_ids: 0,
            midi_length: 0.0f32,
            prerenderer: PrerenderAudio::new(60.0, play_state.clone()),
            stream: None,
            midi_loaded: false,
            stream_playing: false,
            time_nav_changed: false,
            sf_selected: 0,
            sf_loaded: false
        };

        //win.prerenderer.xsynth_load_sfs(&["./test/Kryo Keys II (SF2).sf2"]);
        //win.prerenderer.play();
        win.stream = Some(win.prerenderer.construct_stream());

        win.sync_settings();
        win.init(play_state);
        win
    }

    // updates all variables used in the program to match all setting structs if not done already
    pub fn sync_settings(&mut self) -> () {
        self.prerenderer.xsynth_load_sfs(&self.audio_settings.soundfont_paths);
        self.prerenderer.audio_fps = self.audio_settings.audio_fps;
        self.prerenderer.xsynth_set_layer_count(self.audio_settings.layer_count as usize);

        self.sf_loaded = true;
    }

    fn init(&mut self, global_time: Arc<Mutex<GlobalTimer>>) -> () {
        let ws = LogicalSize::new(
            self.width as f32,
            self.height as f32
        );
        
        let mut event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(self.title)
            .with_inner_size(ws);

        let gl_context = ContextBuilder::new()
            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 3)))
            .build_windowed(window, &event_loop)
            .expect("cant make windowed context");

        let gl_context = unsafe {
            gl_context
                .make_current()
                .expect("failed to make current context")
        };

        gl::load_with(|ptr| gl_context.get_proc_address(ptr) as *const _);
        
        let mut renderer: Renderer = unsafe { Renderer::new(self.width as f32, self.height as f32) };
        renderer.bar_color = self.visual_settings.bar_color;
        renderer.background_color = self.visual_settings.background_color;
        renderer.set_colors(
            self.color_palettes.get_color_table_from_palette_idx(self.visual_settings.palette_index)
        );

        //renderer.set_notes(notes);

        // imgui

        let mut imgui = Context::create();
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), gl_context.window(), HiDpiMode::Default);
        imgui.fonts().build_alpha8_texture();

        let ui_renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| gl_context.get_proc_address(s) as _);
        //let mut pause_time = Instant::now();

        let mut last_frame = Instant::now();

        // some temp stuff
        let mut force_pause = true;

        let a_self = Arc::new(Mutex::new(self));
        event_loop.run_return(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            platform.handle_event(imgui.io_mut(), &gl_context.window(), &event);

            match &event {
                Event::NewEvents(_) => {
                    let now = Instant::now();
                    imgui.io_mut().update_delta_time(now - last_frame);
                    last_frame = now;
                },
                Event::LoopDestroyed => (),
                Event::WindowEvent { 
                    event: WindowEvent::MouseInput { .. },
                    ..
                } => {
                    platform.handle_event(imgui.io_mut(), gl_context.window(), &event);
                },
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::CloseRequested => {
                            (*a_self.lock().unwrap()).stream.as_ref().unwrap().pause().unwrap();
                            (*a_self.lock().unwrap()).stream = None;
                            (*(*a_self.lock().unwrap()).prerenderer.
                                reset_requested.lock().unwrap()) = true;
                            (a_self.lock().unwrap()).save_all_settings();
                            *control_flow = ControlFlow::Exit
                        },
                        WindowEvent::Resized(physical_size) => {
                            gl_context.resize(*physical_size);
                            //gl_context.window().set_inner_size(physical_size);
                            platform.attach_window(imgui.io_mut(), gl_context.window(), HiDpiMode::Default);
                            //platform.scale_size_from_winit(gl_context.window(), physical_size.to_logical(1.0));
                        },
                        WindowEvent::KeyboardInput { input: KeyboardInput {
                                virtual_keycode,
                                state,
                                ..
                            },
                            ..
                        } => {
                            match virtual_keycode  {
                                Some(VirtualKeyCode::Space) => {
                                    if *state == ElementState::Pressed {
                                        let mut g_time = global_time.lock().unwrap();
                                        if g_time.paused {
                                            g_time.play();
                                            force_pause = false;
                                        } else {
                                            g_time.pause();
                                            force_pause = true;
                                        }
                                    }
                                },
                                Some(VirtualKeyCode::Left) => {
                                    if *state == ElementState::Pressed {
                                        let mut g_time = global_time.lock().unwrap();
                                        let cur_time = (-3.0f32).max((*g_time).get_time() - 10.0);
                                        (*g_time).navigate(cur_time);
                                        renderer.time_changed = true;
                                        (a_self.lock().unwrap()).prerenderer.play_audio(g_time.get_time(), g_time.speed, false);
                                        
                                    }
                                },
                                Some(VirtualKeyCode::Right) => {
                                    if *state == ElementState::Pressed {
                                        let mut g_time = global_time.lock().unwrap();
                                        let cur_time = (a_self.lock().unwrap().midi_length).min((*g_time).get_time() + 10.0);
                                        (*g_time).navigate(cur_time);
                                        (a_self.lock().unwrap()).prerenderer.play_audio(g_time.get_time(), g_time.speed, false);
                                    }
                                }
                                _ => {}
                            }
                        },
                        _ => {
                            
                        },
                    }
                },
                Event::RedrawRequested(_) => {
                    platform.prepare_frame(imgui.io_mut(), &gl_context.window())
                        .expect("cannot prepare frame");
                }
                event => {
                    unsafe {
                        let mut g_time = global_time.lock().unwrap();
                        //renderer.time = st.elapsed().as_secs_f32();
                        /*if *player_state == PlayerState::Paused {
                            renderer.time = pause_time.duration_since(st).as_secs_f32() + start_time;
                        } else {
                            renderer.time = pause_time.duration_since(st).as_secs_f32() - st.elapsed().as_secs_f32() + start_time;
                        }*/

                        if (a_self.lock().unwrap()).midi_loaded && !(a_self.lock().unwrap()).stream_playing {
                            (a_self.lock().unwrap()).stream.as_ref().unwrap().play().unwrap();
                            (a_self.lock().unwrap()).prerenderer.play_audio(g_time.get_time(), g_time.speed, true);
                            (a_self.lock().unwrap()).stream_playing = true;
                        }

                        /*if !stream_started {
                            let status = (a_self.lock().unwrap()).stream.play();
                            (*g_time).play();
                            match status {
                                Ok(_) => {

                                },
                                Err(_) => {
                                    *control_flow = ControlFlow::Exit;
                                }
                            }
                            stream_started = true;
                        }*/

                        renderer.time = (*g_time).get_time();
                        renderer.draw(&gl_context);

                        let ui = imgui.frame();

                        (a_self.lock().unwrap()).render_ui(&mut renderer, ui, &mut g_time, &mut force_pause);

                        platform.prepare_render(ui, &gl_context.window());
                        //platform.handle_event(imgui.ihttps://docs.rs/image/latest/image/index.htmlo_mut(), gl_context.window(), &event);
                        //let draw_data = imgui.render();
                        ui_renderer.render(&mut imgui);

                        gl_context.swap_buffers().unwrap();
                    }
                }
            }
            //imgui_winit_support::handle_event(&mut imgui, event);
        });
    }

    fn format_time(&mut self, time_secs: f32) -> String {
        format!("{}{}:{:05.2}", 
            if time_secs < 0.0 {
                "-"
            } else {
                ""
            },
            (time_secs / 60.0).abs() as usize,
            time_secs.abs() % 60.0
        )
    }

    fn render_ui(&mut self, renderer: &mut Renderer, ui: &mut Ui, g_time: &mut GlobalTimer, force_pause: &mut bool) -> () {
        if self.player_settings.show_ui {
            ui.window("midi_info").no_inputs().no_decoration()
            .position([10.0, 25.0], imgui::Condition::Always)
            .always_auto_resize(true)
            .build(|| {
                ui.text(format!("Time: {} / {}", self.format_time(renderer.time), self.format_time(self.midi_length)));
                ui.text(format!("Notes: {} / {}", renderer.notes_passed, renderer.note_count));
                ui.text(format!("Polyphony: {}", renderer.polyphony));
                ui.text(format!("Buffer Length: {}", 
                    self.format_time(self.prerenderer.get_buffer_seconds())
                ));
            });
        }

        ui.main_menu_bar(|| {
            // menu bar
            {
                ui.menu("File", || {
                    if ui.menu_item("Load MIDI") {
                        self.load_midi(renderer, g_time, force_pause);
                    }
                    if ui.menu_item("Unload Current MIDI") {
                        self.unload_midi(renderer, g_time, force_pause);
                    }
                });

                ui.menu("Edit", || {
                    if ui.menu_item("Preferences...") {
                        self.popup_ids |= 0b1;
                    }
                });

                ui.menu("View", || {
                    ui.checkbox("Show UI", &mut self.player_settings.show_ui);
                });
            }

            // navigation
            ui.text("-------");
            // note size
            ui.set_next_item_width(100.0);
            ui.slider("Size", 0.1, 2.0, &mut renderer.note_size);
            ui.set_next_item_width(-1.0);
            if ui.slider("", -3.0, self.midi_length, &mut renderer.time) {
                if !g_time.paused && *force_pause {
                    g_time.pause();
                }
                g_time.navigate(renderer.time);
                renderer.time_changed = true;
                self.time_nav_changed = true;
            } else {
                if g_time.paused && !*force_pause {
                    g_time.play();
                }
                if self.time_nav_changed {
                    self.prerenderer.play_audio(g_time.get_time(), g_time.speed, false);
                    self.time_nav_changed = false;
                }
            }
        });

        // the popups
        // preferences
        if self.popup_ids & 0b1 == 1 {
            ui.window("Preferences").build(|| {
                let tb = ui.tab_bar("pref_tabs").unwrap();
                {
                    // visual tab
                    if let Some(vis) = ui.tab_item("Visual") {
                        if ui.color_edit3("Bar Color", &mut self.visual_settings.bar_color) {
                            renderer.bar_color = self.visual_settings.bar_color;
                        }
                        if ui.color_edit3("Background Color", &mut self.visual_settings.background_color) {
                            renderer.background_color = self.visual_settings.background_color;
                        }
                        /*if ui.button("Load color palette from Image") {
                            *force_pause = true;
                            g_time.pause();
                            let res = self.load_color_palette_from_image();
                            match res {
                                Ok(colors) => { renderer.note_color_table = colors }
                                Err(msg) => {
                                    println!("{}", msg);
                                } 
                            }
                            g_time.play();
                            *force_pause = false;
                        }*/
                        let mut color_palettes_: Vec<_> = self.color_palettes.palette_paths.iter().map(String::as_str).collect();
                        let mut palette_idx = self.visual_settings.palette_index as i32;
                        if ui.list_box("Palettes", &mut palette_idx, &mut color_palettes_, 15) {
                            self.visual_settings.palette_index = palette_idx as usize;
                            renderer.set_colors(
                                self.color_palettes.get_color_table_from_palette_idx(palette_idx as usize)
                            );
                        }
                        if ui.button("Random colors") {
                            renderer.note_color_table.clear();
                            for _ in 0..32 {
                                renderer.note_color_table.push(rand::random::<u32>() & 0xFFFFFF);
                            }
                        }
                        ui.same_line();
                        if ui.button("Shuffle colors") {
                            let mut rng = rand::thread_rng();
                            renderer.note_color_table.shuffle(&mut rng);
                        }
                        vis.end();
                    }

                    if let Some(aud) = ui.tab_item("Audio") {
                        /*if ui.slider("Layer count", 0, 100, &mut lyr_count) {

                        }*/
                        ui.input_int("Layer Count", &mut self.audio_settings.layer_count).build();
                        let sf_act_group = ui.begin_group(); 
                        if ui.button("+") {
                            let file_dialog = FileDialog::new()
                                .add_filter("Soundfont File", &["sf2", "sfz"])
                                .set_title("Add a soundfont");
                            let path = file_dialog.pick_file().unwrap();
                            let sf = path.to_str().unwrap_or("");
                            if sf != "" {
                                self.audio_settings.soundfont_paths.push(String::from(sf));
                                self.sf_loaded = false;
                            }
                        }
                        if ui.button("-") {
                            self.audio_settings.soundfont_paths.remove(self.sf_selected as usize);
                            self.sf_loaded = false;
                        }
                        if ui.button("^") {
                            if self.sf_selected > 0 {
                                self.audio_settings.soundfont_paths.swap(self.sf_selected as usize, self.sf_selected as usize - 1);
                                self.sf_selected -= 1;
                                self.sf_loaded = false;
                            }
                        }
                        if ui.button("v") {
                            if self.sf_selected < self.audio_settings.soundfont_paths.len() as i32 - 1 {
                                self.audio_settings.soundfont_paths.swap(self.sf_selected as usize, self.sf_selected as usize + 1);
                                self.sf_selected += 1;
                                self.sf_loaded = false;
                            }
                        }
                        sf_act_group.end();
                        ui.same_line();
                        let mut sf_list_str: Vec<_> = self.audio_settings.soundfont_paths.iter().map(String::as_str).collect();
                        ui.list_box("Soundfonts", &mut self.sf_selected, 
                            &mut sf_list_str, 15);
                        //let _ = ui.input_int("Layer Count", &mut lyr_count);

                        if ui.button("Load Soundfonts") && !self.sf_loaded {
                            self.prerenderer.xsynth_load_sfs(&self.audio_settings.soundfont_paths);
                            self.sf_loaded = true;
                        }

                        if ui.input_float("Audio FPS", &mut self.audio_settings.audio_fps).build() {
                            self.prerenderer.audio_fps = self.audio_settings.audio_fps;
                        }
                        if self.audio_settings.audio_fps < 1.0 {
                            self.audio_settings.audio_fps = 0.0;
                        }
                        
                        ui.new_line();
                        ui.text("Limiter settings");
                        if ui.input_float("Attack (s)", &mut self.audio_settings.limiter_attack).build() {
                            self.prerenderer.limiter.lock().unwrap().attack
                                = self.audio_settings.limiter_attack * self.prerenderer.sample_rate;
                        }
                        if ui.input_float("Release (s)", &mut self.audio_settings.limiter_release).build() {
                            self.prerenderer.limiter.lock().unwrap().falloff
                                = self.audio_settings.limiter_release * self.prerenderer.sample_rate;
                        }

                        aud.end();
                    }
                }
                tb.end();
                ui.new_line();
                if ui.button("   ok   ") {
                    // apply the settings
                    if !self.sf_loaded {
                        self.prerenderer.xsynth_load_sfs(&self.audio_settings.soundfont_paths);
                        self.sf_loaded = true;
                    }

                    self.prerenderer.xsynth_set_layer_count(self.audio_settings.layer_count as usize);
                    self.prerenderer.play_audio(g_time.get_time(), g_time.speed, true);
                    self.popup_ids ^= 0b1;
                }
            });
        }
    }
    fn load_midi(&mut self, renderer: &mut Renderer, g_time: &mut GlobalTimer, force_pause: &mut bool) {
        let file_diag = FileDialog::new()
            .add_filter("MIDI File", &["mid","midi"])
            .set_title("Open a MIDI File");
        if let Some(path) = file_diag.pick_file() {
            if self.midi_loaded {
                self.unload_midi(renderer, g_time, force_pause);
            }

            let mid: MIDIFile = MIDIFile::new(String::from(path.to_str().unwrap())).unwrap();
            let (mut evs, notes): (Vec<MIDIEvent>, Vec<Vec<Note>>) = mid.get_evs_and_notes();

            renderer.set_notes(notes);
            renderer.time = -3.0;
            g_time.play();
            *force_pause = false;
            self.midi_length = evs.last().unwrap().time;

            self.prerenderer.set_midi_events(evs);
            self.midi_loaded = true;
        }
    }

    fn unload_midi(&mut self, renderer: &mut Renderer, g_time: &mut GlobalTimer, force_pause: &mut bool) {
        if self.midi_loaded == true {
            renderer.set_notes(Vec::new());
            renderer.time = 0.0;
            //self.stream.pause().unwrap();
            *self.prerenderer.reset_requested.lock().unwrap() = true;
            //self.stream.as_ref().unwrap().pause().unwrap();
            self.stream_playing = false;
            g_time.reset();
            g_time.navigate(-3.0);
            renderer.time_changed = true;
            *force_pause = true;
            self.prerenderer.stop();
            self.midi_loaded = false;
            println!("midi unloaded");
        } else {
            println!("cannot unload nothing!");
        }
    }

    fn load_color_palette_from_image(&mut self) -> Result<Vec<u32>, &'static str> {
        let file_diag = FileDialog::new()
            .add_filter("Image File", &["png"])
            .set_title("Open a Color Palette");
        if let Some(path) = file_diag.pick_file() {
            let f = std::fs::File::open(path).unwrap();
            let image = ImageReader::new(
                BufReader::new(
                    f
                ))
                .with_guessed_format()
                .unwrap();

            let dec = image.decode().unwrap();

            if dec.width() != 16 {
                return Err("Image width isn't 16, silly!");
            }

            let mut colors: Vec<u32> = Vec::new();
            for chan in 0..16 {
                for y in 0..dec.height() {
                    let pix = dec.get_pixel(chan, y).0;
                    let pix_u32 =
                        (pix[0] as u32) | 
                        ((pix[1] as u32) << 8) |
                        ((pix[2] as u32) << 16);
                    colors.push(pix_u32);
                }
            }
            Ok(colors)
        } else {
            Err("File dialog closed")
        }
    }

    pub fn save_all_settings(&mut self) {
        self.visual_settings.save_settings();
        self.audio_settings.save_settings();
        self.player_settings.save_settings();
    }
}