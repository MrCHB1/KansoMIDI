use std::{error::Error, fmt, fs::File, io::BufReader, path::{absolute, PathBuf}, sync::atomic::{AtomicI32, Ordering}, time::{Duration, Instant}};

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
    audio::prerender_audio::PrerenderAudio, midi::{midi_file::MIDIFile, midi_track_parser::{MIDIEvent, Note}}, rendering::renderer::Renderer, settings::{advanced_settings::AdvancedSettings, audio_settings::AudioSettings, player_settings::PlayerSettings, visual_settings::VisualSettings}, util::{color_palettes::ColorPalettes, global_timer::GlobalTimer, misc::open_directory_in_explorer}, PlayerState
};
use std::sync::{Arc, Mutex};
use imgui::{Context, InputTextFlags, Ui};

use image::{GenericImageView, ImageReader, Rgb};
use rfd::FileDialog;

pub struct MainWindow {
    pub width: usize,
    pub height: usize,
    pub visual_settings: VisualSettings,
    pub audio_settings: AudioSettings,
    pub player_settings: PlayerSettings,
    pub advanced_settings: AdvancedSettings,
    title: &'static str,

    popup_ids: u16,
    popup_help_title: &'static str,
    popup_help_text: &'static str,
    midi_length: f32,
    prerenderer: PrerenderAudio,
    stream: Option<cpal::Stream>,

    color_palettes: ColorPalettes,

    midi_loaded: bool,
    stream_playing: bool,
    time_nav_changed: bool,
    fps: Arc<AtomicI32>,
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
        let mut advanced_settings = AdvancedSettings::new();

        visual_settings.load_settings();
        audio_settings.load_settings();
        player_settings.load_settings();
        advanced_settings.load_settings();

        let key_threads = advanced_settings.per_key_thread_count;
        let channel_threads = advanced_settings.per_chan_thread_count;
        
        let mut win = Self {
            width,
            height,
            title,

            visual_settings,
            audio_settings,
            player_settings,
            advanced_settings,

            color_palettes: ColorPalettes::new(),

            popup_ids: 0,
            popup_help_title: "Help dialog",
            popup_help_text: "Help text",
            midi_length: 0.0f32,
            prerenderer: PrerenderAudio::new(
                60.0,
                play_state.clone(),
                key_threads,
                channel_threads
            ),
            stream: None,
            midi_loaded: false,
            stream_playing: false,
            time_nav_changed: false,
            fps: Arc::new(AtomicI32::new(0)),
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

        match self.visual_settings.keyboard_range_id {
            0 => {
                renderer.first_key = 21;
                renderer.last_key = 108;
            },
            1 => {
                renderer.first_key = 0;
                renderer.last_key = 127;
            },
            2 => {
                renderer.first_key = 0;
                renderer.last_key = 255;
            },
            3 => {
                renderer.first_key = 0;
                renderer.last_key = 127;
            },
            4 => {
                renderer.first_key = self.visual_settings.kb_first_key;
                renderer.last_key = self.visual_settings.kb_last_key;
            },
            _ => {
                panic!("Unknown Keyboard Range: {}", self.visual_settings.keyboard_range_id)
            }
        }
        
        //renderer.set_notes(notes);

        // imgui

        let mut imgui = Context::create();
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), gl_context.window(), HiDpiMode::Default);
        imgui.fonts().build_alpha8_texture();

        let ui_renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| gl_context.get_proc_address(s) as _);
        //let mut pause_time = Instant::now();

        let mut last_frame = Instant::now();

        // for fps calculation
        let mut fps_start = Instant::now();
        let mut num_frames: usize = 0;

        // for fps limiting
        let mut render_time = Instant::now();
        let mut frame_can_render: bool = false;

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
                            *control_flow = ControlFlow::Exit;
                            (*a_self.lock().unwrap()).prerenderer.stop();
                            (*a_self.lock().unwrap()).stream = None;
                            (a_self.lock().unwrap()).save_all_settings();
                        },
                        WindowEvent::Resized(physical_size) => {
                            gl_context.resize(*physical_size);
                            //gl_context.window().set_inner_size(physical_size);
                            platform.attach_window(imgui.io_mut(), gl_context.window(), HiDpiMode::Default);
                            //platform.scale_size_from_winit(gl_context.window(), physical_size.to_logical(1.0));
                            unsafe {
                                renderer.resize(physical_size.width as i32, physical_size.height as i32);
                            }
                        },
                        WindowEvent::KeyboardInput { input: KeyboardInput {
                                virtual_keycode,
                                state,
                                ..
                            },
                            ..
                        } => {
                            if (a_self.lock().unwrap()).popup_ids & 0b1 == 1 { return; }

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
                _event => {
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
                        
                        if render_time.elapsed().as_secs_f32() >= 1.0 / (a_self.lock().unwrap()).advanced_settings.max_fps as f32 {
                            //render_extra_time = render_time_elapsed - 1.0 / (a_self.lock().unwrap()).advanced_settings.max_fps as f32;
                            frame_can_render = true;
                        }
                        if !(a_self.lock().unwrap()).advanced_settings.limit_fps {
                            frame_can_render = true;
                        }

                        if frame_can_render {
                            renderer.time = (*g_time).get_time();
                            renderer.draw(&gl_context);
                            //platform.handle_event(imgui.ihttps://docs.rs/image/latest/image/index.htmlo_mut(), gl_context.window(), &event);
                            //let draw_data = imgui.render();

                            let ui = imgui.frame();
                            (a_self.lock().unwrap()).render_ui(&mut renderer, ui, &mut g_time, &mut force_pause);
                            platform.prepare_render(ui, &gl_context.window());
                            ui_renderer.render(&mut imgui);
                            
                            gl_context.swap_buffers().unwrap();

                            num_frames += 1;
                            frame_can_render = false;
                            render_time = Instant::now();
                        }

                        // update every 1 second
                        let frame_elapsed = fps_start.elapsed().as_secs_f32();
                        if frame_elapsed > 1.0 {
                            (a_self.lock().unwrap()).fps.store((num_frames as f32 / frame_elapsed) as i32, Ordering::SeqCst);
                            num_frames = 0;
                            fps_start = Instant::now();
                        }
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

    // ---- GUI STUFF ----

    fn input_int_with_hint(&mut self, ui: &Ui, label: &'static str, value: &mut i32, help_text: &'static str) -> bool {
        let changed = ui.input_int(label, value).build();
        ui.same_line();
        if ui.button("?") {
            self.popup_help_title = label;
            self.popup_help_text = help_text;
            self.popup_ids |= 0b10;
        }
        changed
    }

    fn checkbox_with_hint(&mut self, ui: &Ui, label: &'static str, value: &mut bool, help_text: &'static str) -> bool {
        let changed = ui.checkbox(label, value);
        ui.same_line();
        if ui.button("?") {
            self.popup_help_title = label;
            self.popup_help_text = help_text;
            self.popup_ids |= 0b10;
        }
        changed
    }

    fn render_meta_stats_ui(&mut self, renderer: &Renderer, ui: &Ui) -> () {
        ui.window("meta_info").no_inputs().no_decoration()
            .position_pivot([1.0,0.0])
            .size([200.0, 0.0], imgui::Condition::Once)
            .position([renderer.width - 10.0, 25.0], imgui::Condition::Always)
            .build(|| {
                if !renderer.curr_marker_text.is_empty() {
                    ui.text_wrapped(format!("{}", renderer.curr_marker_text));
                }
            });
    }

    fn render_stats_ui(&mut self, renderer: &Renderer, ui: &Ui) -> () {
        ui.window("midi_info").no_inputs().no_decoration()
            .position([10.0, 25.0], imgui::Condition::Always)
            .always_auto_resize(true)
            .build(|| {
                ui.text(format!("Time: {} / {}", self.format_time(renderer.time), self.format_time(self.midi_length)));
                ui.text(format!("Notes: {} / {}", renderer.notes_passed, renderer.note_count));
                ui.text(format!("Polyphony: {}", renderer.polyphony));
                ui.text(format!("FPS: {}", self.fps.load(Ordering::Relaxed)));
                ui.text(format!("Buffer Length: {}", 
                    self.format_time(self.prerenderer.get_buffer_seconds())
                ));
            });
    }

    fn render_pref_visual_tab(&mut self, renderer: &mut Renderer, ui: &Ui) -> () {
        ui.text("Key Range");
        /* 0: 88 keys, 1: 128 keys, 2: 256 keys, 3: MIDI key range, 4: Custom */
        if ui.radio_button("88 Keys", &mut self.visual_settings.keyboard_range_id, 0) {
            renderer.first_key = 21;
            renderer.last_key = 108;
        }
        if ui.radio_button("128 Keys", &mut self.visual_settings.keyboard_range_id, 1) {
            renderer.first_key = 0;
            renderer.last_key = 127;
        }
        if ui.radio_button("256 Keys", &mut self.visual_settings.keyboard_range_id, 2) {
            renderer.first_key = 0;
            renderer.last_key = 255;
        }
        if ui.radio_button("MIDI's Key Range", &mut self.visual_settings.keyboard_range_id, 3) {

        }
        if ui.radio_button("Custom", &mut self.visual_settings.keyboard_range_id, 4) {
            renderer.first_key = self.visual_settings.kb_first_key;
            renderer.last_key = self.visual_settings.kb_last_key;
        }
        ui.disabled(self.visual_settings.keyboard_range_id != 4, || {
            let mut first_key = self.visual_settings.kb_first_key as i32;
            let mut last_key = self.visual_settings.kb_last_key as i32;
            
            ui.set_next_item_width(85.0);
            if ui.input_int("Low", &mut first_key).build() {
                self.visual_settings.set_kb_first_key(first_key);
                renderer.first_key = self.visual_settings.kb_first_key;
            }
            ui.set_next_item_width(85.0);
            if ui.input_int("High", &mut last_key).build() {
                self.visual_settings.set_kb_last_key(last_key);
                renderer.last_key = self.visual_settings.kb_last_key;
            }
        });

        ui.new_line();
        if ui.color_edit3("Bar Color", &mut self.visual_settings.bar_color) {
            renderer.bar_color = self.visual_settings.bar_color;
        }
        if ui.color_edit3("Background Color", &mut self.visual_settings.background_color) {
            renderer.background_color = self.visual_settings.background_color;
        }
        
        let mut palette_names: Vec<_> = self.color_palettes.palette_names.iter().map(String::as_str).collect();
        let mut palette_idx = self.visual_settings.palette_index as i32;
        if ui.list_box("Palettes", &mut palette_idx, &mut palette_names, 15) {
            self.visual_settings.palette_index = palette_idx as usize;
            renderer.set_colors(
                self.color_palettes.get_color_table_from_palette_idx(palette_idx as usize)
            );
        }
        if ui.button("Open palette folder") {
            open_directory_in_explorer(absolute("./Palettes/").unwrap().to_str().unwrap());
        }
        ui.same_line();
        if ui.button("Reload palettes") {
            self.color_palettes.reload_palette_paths();
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
    }

    fn render_pref_audio_tab(&mut self, renderer: &mut Renderer, ui: &Ui) -> () {
        let mut lyr_count = self.audio_settings.layer_count;
        if self.input_int_with_hint(ui, "Layer Count", &mut lyr_count, "One layer equals 128 voices.") {
            self.audio_settings.layer_count = lyr_count;
        }

        // soundfont box thing
        
        let sf_act_group = ui.begin_group(); 
        if ui.button("+") {
            let file_dialog = FileDialog::new()
                .add_filter("Soundfont File", &["sf2", "sfz"])
                .set_title("Add a soundfont");
            if let Some(path) = file_dialog.pick_file() {
                let sf = path.to_str().unwrap_or("");
                if sf != "" {
                    self.audio_settings.soundfont_paths.push(String::from(sf));
                    self.sf_loaded = false;
                }
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
    }

    fn render_pref_advanced_tab(&mut self, renderer: &mut Renderer, ui: &Ui) -> () {
        ui.text("Settings marked with a <!> requires a restart for the changes to take effect.");
        ui.new_line();
        let mut limit_fps = self.advanced_settings.limit_fps;
        let mut per_key_thread_count = self.advanced_settings.per_key_thread_count as i32;
        let mut per_chan_thread_count = self.advanced_settings.per_chan_thread_count as i32;

        if self.checkbox_with_hint(ui, "Limit FPS", &mut limit_fps, "Disabling this option allows for smoother playback at the cost of higher CPU usage.") {
            self.advanced_settings.limit_fps = limit_fps;
        }
        
        //ui.checkbox("Limit FPS", &mut self.advanced_settings.limit_fps);
        ui.disabled(!self.advanced_settings.limit_fps, || {
            let mut max_fps = self.advanced_settings.max_fps as i32;
            if ui.input_int("Max FPS", &mut max_fps).build() {
                self.advanced_settings.set_max_fps(max_fps);
            }
        });

        ui.new_line();
        if self.input_int_with_hint(ui, "Per Key Thread Count <!>", &mut per_key_thread_count, "How many threads XSynth should use for each key while rendering audio.\nA value of zero means an automatic thread count.") {
            //self.advanced_settings.per_key_thread_count = per_key_thread_count;
            self.advanced_settings.set_per_key_thread_count(per_key_thread_count);
        }
        if self.input_int_with_hint(ui, "Per Channel Thread Count <!>", &mut per_chan_thread_count, "How many threads XSynth should use for each MIDI channel while rendering audio.\nA value of zero means an automatic thread count.") {
            //self.advanced_settings.per_chan_thread_count = per_chan_thread_count;
            self.advanced_settings.set_per_chan_thread_count(per_chan_thread_count);
        }
    }

    fn render_pref_misc_tab(&mut self, renderer: &mut Renderer, ui: &Ui) -> () {
        ui.text("Settings marked with an asterisk (*) will not be saved.");
        ui.new_line();

        let mut transpose = self.audio_settings.misc_transpose;
        if ui.input_int("Transpose *", &mut transpose).build() {
            let mut restart_audio = false;
            self.audio_settings.misc_transpose = if transpose < -12 {
                -12
            } else if transpose > 12 {
                12
            } else {
                restart_audio = true;
                transpose
            };
            renderer.notes_transpose = transpose;
            self.prerenderer.transpose = transpose;
            renderer.refresh_render_notes();
        }
    }

    fn render_ui(&mut self, renderer: &mut Renderer, ui: &mut Ui, g_time: &mut GlobalTimer, force_pause: &mut bool) -> () {
        if self.player_settings.show_ui {
            /*ui.window("midi_info").no_inputs().no_decoration()
            .position([10.0, 25.0], imgui::Condition::Always)
            .always_auto_resize(true)
            .build(|| {
                ui.text(format!("Time: {} / {}", self.format_time(renderer.time), self.format_time(self.midi_length)));
                ui.text(format!("Notes: {} / {}", renderer.notes_passed, renderer.note_count));
                ui.text(format!("Polyphony: {}", renderer.polyphony));
                ui.text(format!("FPS: {}", self.fps.load(Ordering::Relaxed)));
                ui.text(format!("Buffer Length: {}", 
                    self.format_time(self.prerenderer.get_buffer_seconds())
                ));
            });*/
            self.render_stats_ui(renderer, ui);
            //self.render_meta_stats_ui(renderer, ui);
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
            ui.text("                ");
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
        if self.popup_ids & 0b1 == 0b1 {
            ui.window("Preferences").build(|| {
                let tb = ui.tab_bar("pref_tabs").unwrap();
                {
                    // visual tab
                    if let Some(vis) = ui.tab_item("Visual") {
                        self.render_pref_visual_tab(renderer, ui);
                        vis.end();
                    }

                    // audio tab
                    if let Some(aud) = ui.tab_item("Audio") {
                        self.render_pref_audio_tab(renderer, ui);
                        aud.end();
                    }

                    if let Some(adv) = ui.tab_item("Advanced") {
                        self.render_pref_advanced_tab(renderer, ui);
                        adv.end();
                    }

                    if let Some(misc) = ui.tab_item("Miscellaneous") {
                        self.render_pref_misc_tab(renderer, ui);
                        misc.end();
                    }
                }
                tb.end();
                ui.new_line();
                if ui.button("   ok   ") {
                    // apply the settings
                    self.prerenderer.stop();

                    if !self.sf_loaded {
                        self.prerenderer.xsynth_load_sfs(&self.audio_settings.soundfont_paths);
                        self.sf_loaded = true;
                        println!("loaded soundfonts successfully");
                    }

                    self.prerenderer.xsynth_set_layer_count(self.audio_settings.layer_count as usize);
                    self.prerenderer.play_audio(g_time.get_time(), g_time.speed, true);
                    self.popup_ids ^= 0b1;
                }
            });
        }
    
        // help dialog
        if self.popup_ids & 0b10 == 0b10 {
            ui.window(self.popup_help_title)
                .always_auto_resize(true)
                .focused(true)
                .build(|| {
                ui.text(self.popup_help_text);
                ui.new_line();
                if ui.button("   ok   ") {
                    self.popup_ids ^= 0b10;
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
            //let (mut evs, notes): (Vec<MIDIEvent>, Vec<Vec<Note>>) = mid.get_evs_and_notes();
            let mut evs: Vec<MIDIEvent> = Vec::new();
            let mut notes: Vec<Vec<Note>> = Vec::new();
            mid.get_sequences(&mut evs, &mut notes);

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
            self.prerenderer.reset_requested.store(true, Ordering::SeqCst);
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
        self.advanced_settings.save_settings();
    }
}