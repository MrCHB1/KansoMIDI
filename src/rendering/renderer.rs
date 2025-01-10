use gl::{self};
use glutin::{window::Window, ContextWrapper, PossiblyCurrent};
use itertools::Itertools;
use core::str;
use std::{fs::{create_dir, File}, io::{Read, Write}, path::absolute};
use crate::{midi::midi_track_parser::{MetaEvent, MetaEventName, Note}, rendering::{buffers::*, shader::*}, set_attribute};

// random color!!!
use rand::prelude::*;

pub type TexCoord = [f32; 2];

const NOTE_BUFFER_SIZE: usize = 4096;
const USE_RANDOM_COLORS: bool = true;

#[repr(C, packed)]
pub struct Vertex(TexCoord);

// render note
pub type NoteTimes = [f32; 2];
pub type NoteColors = [f32; 3];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderNote(NoteTimes, NoteColors);

// render key
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderKey {
    color: u32,
    meta: u32,
    key_weight: f32,
}

impl RenderKey {
    pub fn mark_pressed(&mut self, pressed: bool) {
        self.meta = self.meta & 0b11111101;
        if pressed {
            self.meta = self.meta | 0b10;
        }
    }

    pub fn set_key_weight(&mut self, velocity: u8) {
        self.key_weight = 1.0 - (velocity as f32) / 127.0;
    }

    pub fn mark_black(&mut self, black: bool) {
        self.meta = self.meta & 0b11111110;
        if black {
            self.meta = self.meta | 0b1;
        }
    }
}

#[rustfmt::skip]
pub const NOTE_VERTICES: [Vertex; 8] = [
    Vertex([0.0, 0.0]),
    Vertex([1.0, 0.0]),
    Vertex([1.0, 1.0]),
    Vertex([0.0, 1.0]),
    Vertex([0.0, 0.0]),
    Vertex([1.0, 0.0]),
    Vertex([1.0, 1.0]),
    Vertex([0.0, 1.0])
];

const NOTE_INDICES: [u32; 12] = [
    0, 1, 3,
    1, 2, 3,
    4, 5, 7,
    5, 6, 7
];

const QUAD_VERTICES: [Vertex; 4] = [
    Vertex([0.0, 0.0]),
    Vertex([1.0, 0.0]),
    Vertex([1.0, 1.0]),
    Vertex([0.0, 1.0])
];

const QUAD_INDICES: [u32; 6] = [
    0, 1, 3,
    1, 2, 3
];

pub struct Renderer {
    note_buffer_size: usize,

    pub n_program: ShaderProgram,
    n_vertex_buffer: Buffer,
    n_vertex_array: VertexArray,
    n_instance_buffer: Buffer,
    n_index_buffer: Buffer,

    pub k_program: ShaderProgram,
    k_vertex_buffer: Buffer,
    k_vertex_array: VertexArray,
    k_index_buffer: Buffer,
    k_instance_buffer: Buffer,

    pub b_program: ShaderProgram,
    b_vertex_buffer: Buffer,
    b_vertex_array: VertexArray,
    b_index_buffer: Buffer,

    pub render_notes: Vec<Vec<Note>>,
    pub notes_render: Vec<RenderNote>,
    pub note_color_table: Vec<u32>,
    pub note_color_index_table: Vec<usize>,
    pub time: f32,
    pub time_changed: bool,
    last_note_starts: [usize; 257],

    pub width: f32,
    pub height: f32,

    pub first_key: usize,
    pub last_key: usize,
    black_keys: [bool; 257],
    key_num: [usize; 257],
    black_key_ids: Vec<usize>,
    white_key_ids: Vec<usize>,
    render_keys: [RenderKey; 257],
    x1array: [f32; 257],
    wdtharray: [f32; 257],

    pub notes_passed: usize,
    pub note_count: usize,
    pub polyphony: usize,
    first_unhit_note: [usize; 257],

    // experimental thing
    kb_key_velocities: [u8; 257],

    pub note_size: f32,
    pub bar_color: [f32; 3],
    pub background_color: [f32; 3],
    pub notes_transpose: i32,

    // meta events
    pub meta_events: Vec<MetaEvent>,
    pub meta_passed: usize,
    pub curr_marker_text: String
}

impl Renderer {
    pub unsafe fn new(width: f32, height: f32) -> Self {
        verify_shader_integrity();
        // notes shader
        let mut n_vertex_source = String::new();
        File::open(absolute("./shaders/n_vertex.glsl").unwrap()).unwrap()
            .read_to_string(&mut n_vertex_source)
            .unwrap();

        let mut n_fragment_source = String::new();
        File::open(absolute("./shaders/n_fragment.glsl").unwrap()).unwrap()
            .read_to_string(&mut n_fragment_source)
            .unwrap();
            
        let n_vertex = Shader::new(
            &n_vertex_source, 
            gl::VERTEX_SHADER
        ).unwrap();

        let n_fragment = Shader::new(
            &n_fragment_source,
            gl::FRAGMENT_SHADER
        ).unwrap();

        let n_program = ShaderProgram::new(
            &[n_vertex, n_fragment]
        ).unwrap();

        // keyboard shader
        let mut k_vertex_source = String::new();
        File::open(absolute("./shaders/k_vertex.glsl").unwrap()).unwrap()
            .read_to_string(&mut k_vertex_source)
            .unwrap();
        
        let mut k_fragment_source = String::new();
        File::open(absolute("./shaders/k_fragment.glsl").unwrap()).unwrap()
            .read_to_string(&mut k_fragment_source)
            .unwrap();

        let k_vertex = Shader::new(
            &k_vertex_source,
            gl::VERTEX_SHADER
        ).unwrap();

        let k_fragment = Shader::new(
            &k_fragment_source,
            gl::FRAGMENT_SHADER
        ).unwrap();

        let mut k_program = ShaderProgram::new(
            &[k_vertex, k_fragment]
        ).unwrap();
        
        // bar shader

        let mut b_vertex_source = String::new();
        File::open(absolute("./shaders/b_vertex.glsl").unwrap()).unwrap()
            .read_to_string(&mut b_vertex_source)
            .unwrap();

        let b_vertex = Shader::new(
            &b_vertex_source,
            gl::VERTEX_SHADER
        ).unwrap();

        let mut b_fragment_source = String::new();
        File::open(absolute("./shaders/b_fragment.glsl").unwrap()).unwrap()
            .read_to_string(&mut b_fragment_source)
            .unwrap();

        let b_fragment = Shader::new(
            &b_fragment_source,
            gl::FRAGMENT_SHADER
        ).unwrap();
        
        let b_program = ShaderProgram::new(
            &[b_vertex, b_fragment]
        ).unwrap();
        
        // note resources

        let note_vertex_buffer = Buffer::new(gl::ARRAY_BUFFER);
        note_vertex_buffer.set_data(&NOTE_VERTICES, gl::STATIC_DRAW);

        let note_index_buffer = Buffer::new(gl::ELEMENT_ARRAY_BUFFER);
        note_index_buffer.set_data(&NOTE_INDICES, gl::STATIC_DRAW);

        let note_vertex_array = VertexArray::new();
        let uv_attrib = n_program.get_attrib_location("texcoord").unwrap();
        set_attribute!(gl::FLOAT, note_vertex_array, uv_attrib, Vertex::0);
        
        let note_instance_array = Buffer::new(gl::ARRAY_BUFFER);
        let notes_render = [RenderNote {
            0:[0.0, 1.0],
            1:[0.0, 0.0, 0.0]
        }; NOTE_BUFFER_SIZE];
        note_instance_array.set_data(notes_render.as_slice(), gl::DYNAMIC_DRAW);

        let note_times_attrib: u32 = n_program.get_attrib_location("note_times").unwrap();
        set_attribute!(gl::FLOAT, note_vertex_array, note_times_attrib, RenderNote::0);
        let note_colors_attrib = n_program.get_attrib_location("colors").unwrap();
        set_attribute!(gl::FLOAT, note_vertex_array, note_colors_attrib, RenderNote::1);
        //note_vertex_array.set_attribute::<u32>(gl::UNSIGNED_INT, note_colors_attrib, 1, 0);

        gl::VertexAttribDivisor(1, 1);
        gl::VertexAttribDivisor(2, 1);

        //gl::BindVertexArray(0);
        //gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        //gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);

        gl::UseProgram(k_program.id);

        // keyboard resources

        let keyboard_vertex_buffer = Buffer::new(gl::ARRAY_BUFFER);
        keyboard_vertex_buffer.set_data(&QUAD_VERTICES, gl::STATIC_DRAW);
        //keyboard_vertex_buffer.unbind();

        let keyboard_index_buffer = Buffer::new(gl::ELEMENT_ARRAY_BUFFER);
        keyboard_index_buffer.set_data(&QUAD_INDICES, gl::STATIC_DRAW);
        //keyboard_index_buffer.unbind();

        let keyboard_vertex_array = VertexArray::new();
        //let k_uv_attrib = k_program.get_attrib_location("texcoord").unwrap();
        //set_attribute!(gl::FLOAT, keyboard_vertex_array, k_uv_attrib, Vertex::0);
        //keyboard_vertex_array.unbind();

        let keyboard_instance_buffer = Buffer::new(gl::ARRAY_BUFFER);
        let mut render_keys = [RenderKey {
            color: 0,
            meta: 0,
            key_weight: 0.0,
        }; 257];
        keyboard_instance_buffer.set_data(render_keys.as_slice(), gl::DYNAMIC_DRAW);
        
        // bar resources
        gl::UseProgram(b_program.id);

        let bar_vertex_buffer = Buffer::new(gl::ARRAY_BUFFER);
        bar_vertex_buffer.set_data(&QUAD_VERTICES, gl::STATIC_DRAW);
        
        let bar_index_buffer = Buffer::new(gl::ELEMENT_ARRAY_BUFFER);
        bar_index_buffer.set_data(&QUAD_INDICES, gl::STATIC_DRAW);

        let bar_vertex_array = VertexArray::new();
        //let uv_attrib = b_program.get_attrib_location("texcoord").unwrap();
        //set_attribute!(gl::FLOAT, bar_vertex_array, uv_attrib, Vertex::0);

        // keyboard stuffs idk

        let mut black_keys: [bool; 257] = [false; 257];
        for i in 0..257 {
            let key_oct = i % 12;
            black_keys[i] = key_oct == 1 || key_oct == 3 || key_oct == 6 || key_oct == 8 || key_oct == 10;
        }

        let mut key_num: [usize; 257] = [0; 257];

        let mut b: usize = 0;
        let mut w: usize = 0;
        let mut black: Vec<usize> = Vec::new();
        let mut white: Vec<usize> = Vec::new();

        for i in 0..257 {
            if black_keys[i] {
                key_num[i] = b;
                b += 1;
                black.push(i);
            } else {
                key_num[i] = w;
                w += 1;
                white.push(i);
            }
        }

        // keyboard stuff lol
        let first_note: isize = 0;
        let last_note: isize = 256;

        let mut wdth: f32 = 0.0;

        let black_key_scale = 0.65;
        let offset2set = 0.3;
        let offset3set = 0.5;

        let mut knmfn = key_num[(first_note) as usize] as f32;
        let mut knmln = key_num[(last_note-1) as usize] as f32;
        if black_keys[(first_note) as usize] && first_note > 0 { knmfn = key_num[(first_note-1) as usize] as f32 + 0.5f32; }
        if black_keys[(last_note-1) as usize] { knmln = key_num[last_note as usize] as f32 - 0.5f32; }

        let mut x1array = [0.0f32; 257];
        let mut wdtharray = [0.0f32; 257];

        for i in 0..257 {
            if !black_keys[i] {
                x1array[i] = (key_num[i] as f32 - knmfn) / (knmln - knmfn + 1.0);
                wdtharray[i] = 1.0 / (knmln - knmfn + 1.0);
            } else {
                let _i = i + 1;
                wdth = black_key_scale / (knmln - knmfn + 1.0);
                let bknum = key_num[i] % 5;
                let mut offset = wdth / 2.0;
                if bknum == 0 { offset += offset * offset2set; }
                if bknum == 2 { offset += offset * offset3set; }
                if bknum == 1 { offset -= offset * offset2set; }
                if bknum == 4 { offset -= offset * offset3set; }

                x1array[i] = (key_num[_i] as f32 - knmfn) / (knmln - knmfn + 1.0) - offset;
                wdtharray[i] = wdth;
                // keyboard later
                render_keys[i].mark_black(true);
            }
            //render_keys[i].left = x1array[i];
            //render_keys[i].right = x1array[i] + wdtharray[i];
        }

        let mut note_color_table = Vec::<u32>::new();

        if USE_RANDOM_COLORS {
            for _ in 0..32 {
                note_color_table.push(random::<u32>() & 0xFFFFFF);
            }
        } else {
            note_color_table = vec![
                0x0000FF,
                0x0080FF,
                0x00FFFF,
                0x00FF00,
                0xFFFF00,
                0xFF0000,
                0xFF0080,
                0xFF00FF
            ];
        }

        Self {
            note_buffer_size: NOTE_BUFFER_SIZE,
            n_program,
            n_vertex_buffer: note_vertex_buffer,
            n_vertex_array: note_vertex_array,
            n_index_buffer: note_index_buffer,
            n_instance_buffer: note_instance_array,

            k_program,
            k_vertex_buffer: keyboard_vertex_buffer,
            k_vertex_array: keyboard_vertex_array,
            k_index_buffer: keyboard_index_buffer,
            k_instance_buffer: keyboard_instance_buffer,

            b_program,
            b_vertex_buffer: bar_vertex_buffer,
            b_vertex_array: bar_vertex_array,
            b_index_buffer: bar_index_buffer,

            render_notes: Vec::new(),
            notes_render: notes_render.to_vec(),
            note_color_table,
            note_color_index_table: (0..16).collect_vec(),
            time: 0.0,
            last_note_starts: [0; 257],
            time_changed: false,

            width,
            height,

            first_key: 0,
            last_key: 256,
            black_keys,
            key_num,
            black_key_ids: black,
            white_key_ids: white,
            render_keys,
            x1array,
            wdtharray,

            kb_key_velocities: [0u8; 257],

            notes_passed: 0,
            note_count: 0,
            polyphony: 0,
            note_size: 0.25,
            first_unhit_note: [0; 257],

            bar_color: [1.0, 0.0, 0.0],
            background_color: [0.5, 0.5, 0.5],
            notes_transpose: 0,

            curr_marker_text: String::new(),
            meta_events: vec![
                MetaEvent {
                    time: 1.0,
                    meta_name: MetaEventName::Marker,
                    data: String::from("You should be able to see this if this works!").as_mut_vec().to_vec()
                }
            ],
            meta_passed: 0
        }
    }

    // this will move notes, be careful
    pub fn set_notes(&mut self, notes: Vec<Vec<Note>>) -> () {
        self.render_notes = notes;
        self.note_count = self.render_notes.iter().map(|n| n.len()).sum();
    }

    pub fn set_colors(&mut self, colors: Vec<u32>) -> () {
        self.note_color_table = colors;
        self.note_color_index_table = (0..self.note_color_table.len()).collect_vec();
    }

    pub fn refresh_render_notes(&mut self) -> () {
        for key in 0..257 {
            self.last_note_starts[key] = 0;
            self.first_unhit_note[key] = 0;
        }
    }

    fn update_meta_info(&mut self) -> () {
        for meta in self.meta_events.iter() {
            if meta.time > self.time { break; }
            match meta.meta_name {
                MetaEventName::Marker => {
                    self.curr_marker_text = String::from_utf8_lossy(&meta.data).to_string();
                },
                _ => {

                }
            }
        }
    }

    pub unsafe fn resize(&mut self, width: i32, height: i32) -> () {
        gl::Viewport(0, 0, width, height);
        self.width = width as f32;
        self.height = height as f32;
    }

    pub unsafe fn draw(&mut self, context: &ContextWrapper<PossiblyCurrent, Window>) -> () {
        self.update_meta_info();
        gl::ClearColor(self.background_color[0], self.background_color[1], self.background_color[2], 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);

        let scale = self.note_size;
        // test
        self.n_program.set_vec2("resolution", self.width, self.height);

        let first_note: usize = self.first_key;
        let last_note: usize = self.last_key + 1;

        let mut kbfirstnote = first_note;
        let mut kblastnote = last_note;
        if self.black_keys[first_note as usize] { kbfirstnote -= 1; }
        if self.black_keys[(last_note-1) as usize] { kblastnote += 1; }

        let full_left = self.x1array[first_note as usize];
        let full_right = self.x1array[(last_note-1) as usize] + self.wdtharray[(last_note-1) as usize];
        let full_width = full_right - full_left;

        let mut kb_height = self.width / self.height / full_width;
        kb_height *= 0.04;

        let mut ids: &mut Vec<usize>;

        self.notes_passed = 0;
        self.polyphony = 0;

        if self.render_notes.len() > 0 {
            for black in 0..2 {
                if black == 1 { ids = &mut self.black_key_ids; }
                else { ids = &mut self.white_key_ids; }

                for &key in ids.iter() {
                    let mut skip_draw: bool = false;
                    let mut real_key = 255 - (key as i32 - self.notes_transpose);
                    if real_key < 0 || real_key > 255 {
                        skip_draw = true;
                    }
                    //real_key = if real_key < 0 { 0 } else if real_key > 255 { 255 } else { real_key };
                    //gl::UseProgram(self.n_program.id);
                    //self.n_program.set_vec2("resolution", self.width, self.height);
                    //self.n_program.set_float("keyboard_height", kb_height);

                    //self.n_index_buffer.bind();
                    let left: f32 = (self.x1array[key] - full_left) / full_width;
                    let right: f32 = (self.x1array[key] + self.wdtharray[key] - full_left) / full_width;
                    let mut pressed = false;
                    let mut color = 0u32;

                    let notes = if skip_draw {
                        &Vec::new()
                    } else {
                        &self.render_notes[real_key as usize]
                    };

                    let mut n_id = 0;
                    let mut max_vel = 0u8;

                    let last_hit_note = self.first_unhit_note[key] - 1;

                    let note_start = {
                        let mut s = if self.time_changed {
                            self.last_note_starts[key] = 0;
                            0
                        } else {
                            self.last_note_starts[key]
                        };

                        for i in s..notes.len() {
                            if notes[i].end as f32 / 1000000.0 > self.time { break; }
                            s += 1;
                        }
                        self.last_note_starts[key] = s;
                        s
                    };

                    let note_end = {
                        let mut e = note_start;
                        for i in note_start..notes.len() {
                            if notes[i].start as f32 / 1000000.0 > self.time + scale { break; }
                            e += 1;
                        }
                        e
                    };

                    self.notes_passed += note_start;

                    if notes.len() > 0 {
                        for n in notes[note_start..note_end].iter() {
                            if n.end as f32 / 1000000.0 < self.time {
                                self.notes_passed += 1;
                                continue;
                            }
                            if n.start as f32 / 1000000.0 < self.time {
                                pressed = true;
                                self.polyphony += 1;
                                color = self.note_color_table[(n.channel as usize * 16 + n.track) % self.note_color_table.len()];
                                self.render_keys[key].color = color;
                                if n.velocity > max_vel {
                                    max_vel = n.velocity;
                                }
                                self.render_keys[key].set_key_weight(max_vel);
                            }
                            if key < kbfirstnote || key > kblastnote - 1 { continue; }

                            if n.end as f32 / 1000000.0 < self.time { continue };
                            if n.start as f32 / 1000000.0 > self.time + scale { continue };

                            let note_color = self.note_color_table[(n.channel as usize * 16 + n.track) % self.note_color_table.len()];

                            self.notes_render[n_id] = RenderNote {
                                0: [(n.start as f32 / 1000000.0 - self.time) / scale,
                                (n.end as f32 / 1000000.0 - self.time) / scale],
                                1: [(note_color & 0xFF) as f32 / 255.0, ((note_color >> 8) & 0xFF) as f32 / 255.0, ((note_color >> 16) & 0xFF) as f32 / 255.0]
                            };
                            n_id += 1;
                            //self.notes_passed += 1;
                            if n_id >= self.note_buffer_size {
                                self.n_program.set_float("keyboard_height", kb_height);
                                self.n_program.set_float("left", left);
                                self.n_program.set_float("right", right);

                                /*gl::BufferSubData(
                                    gl::ARRAY_BUFFER, 
                                    0, 
                                    (std::mem::size_of::<RenderNote>() * self.note_buffer_size) as isize,
                                    self.notes_render.as_ptr() as *const _
                                );*/
                                self.n_instance_buffer.bind();
                                self.n_vertex_buffer.bind();
                                self.n_index_buffer.bind();
                                self.n_instance_buffer.set_data(self.notes_render.as_slice(), gl::DYNAMIC_DRAW);
                                gl::DrawElementsInstanced(
                                    gl::TRIANGLES,
                                    12,
                                    gl::UNSIGNED_INT,
                                    std::ptr::null(),
                                    self.note_buffer_size as i32
                                );
                                n_id = 0;
                            }
                        }
                        self.first_unhit_note[key] = last_hit_note + 1;
                    }

                    if n_id != 0 {
                        self.n_program.set_float("keyboard_height", kb_height);
                        self.n_program.set_float("left", left);
                        self.n_program.set_float("right", right);
                        self.n_instance_buffer.set_data(self.notes_render.as_slice(), gl::DYNAMIC_DRAW);
                        gl::UseProgram(self.n_program.id);
                        /*gl::BufferSubData(
                            gl::ARRAY_BUFFER, 
                            0, 
                            (std::mem::size_of::<RenderNote>() * n_id) as isize,
                            self.notes_render.as_ptr() as *const _
                        );*/
                        self.n_instance_buffer.bind();
                        self.n_vertex_buffer.bind();
                        self.n_index_buffer.bind();
                        gl::DrawElementsInstanced(
                            gl::TRIANGLES,
                            12,
                            gl::UNSIGNED_INT,
                            std::ptr::null(),
                            n_id as i32
                        );
                    }

                    //self.render_keys[key].color = color;
                    self.render_keys[key].mark_pressed(pressed);
                }
            }
        } else {
            for key in kbfirstnote..kblastnote {
                self.render_keys[key].mark_pressed(false);
                self.render_keys[key].color = 0x000000;
                self.render_keys[key].set_key_weight(0);
            }
        }

        // draw bar
        //self.b_vertex_array.bind();
        gl::UseProgram(self.b_program.id);
        self.b_program.set_float("keyboard_height", kb_height);
        self.b_program.set_vec3("color", self.bar_color[0], self.bar_color[1], self.bar_color[2]);
        gl::DrawElementsInstanced(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null(), 1);

        // draw keyboard 
        //self.k_vertex_array.bind();
        for black in 0..2 {
            if black == 1 { ids = &mut self.black_key_ids; }
            else { ids = &mut self.white_key_ids; }

            for &key in ids.iter() {
                if key < kbfirstnote || key > kblastnote - 1 { continue; }
                self.k_program.set_float("keyboard_height", kb_height);
                //self.k_vertex_buffer.bind();
                //self.k_index_buffer.bind();
                //self.k_instance_buffer.bind();
                let left: f32 = (self.x1array[key] - full_left) / full_width;
                let right: f32 = (self.x1array[key] + self.wdtharray[key] - full_left) / full_width;
                //self.render_keys[key].left = left;
                //self.render_keys[key].right = right;
                gl::UseProgram(self.k_program.id);
                self.k_program.set_float("left", left);
                self.k_program.set_float("right", right);
                self.k_program.set_uint("meta", self.render_keys[key].meta);
                self.k_program.set_uint("color", self.render_keys[key].color);
                self.k_program.set_float("key_weight", self.render_keys[key].key_weight);
                gl::DrawElementsInstanced(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null(), 1);
            }

            /*gl::BufferSubData(gl::ARRAY_BUFFER,
                0,
                (std::mem::size_of::<RenderKey>() * ids.len()) as isize,
                self.render_keys.as_ptr() as *const _
            );*/

            //gl::DrawElementsInstanced(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null(), ids.len() as i32);
        }

        gl::UseProgram(self.n_program.id);

        if self.time_changed {
            self.time_changed = false;
        }

        // summarize
        //self.notes_passed = self.first_unhit_note.iter().sum();
    }
}

fn verify_shader_integrity() -> () {
    // create directory if it doesn't even exist
    let mut recreate_all: bool = false;
    if !absolute("./shaders/").unwrap().exists() {
        recreate_all = true;
    }

    if recreate_all { create_dir(absolute("./shaders/").unwrap()).unwrap(); }
    
    let file_paths = [
        "./shaders/n_vertex.glsl",
        "./shaders/n_fragment.glsl",
        "./shaders/k_vertex.glsl",
        "./shaders/k_fragment.glsl",
        "./shaders/b_vertex.glsl",
        "./shaders/b_fragment.glsl"
    ];
    // check if all shaders exist, first
    let needs_recreations = if recreate_all {
        let len = file_paths.len();
        vec![true; len]
    } else {
        file_paths.iter().map(|s| !absolute(s).unwrap().exists()).collect::<Vec<bool>>()
    };
    // raw source files
    let shader_sources: Vec<String> = vec![
String::from(
// n_vertex.glsl
"#version 330

layout (location = 0) in vec2 texcoord;
layout (location = 1) in vec2 note_times;
layout (location = 2) in vec3 colors;

uniform float left;
uniform float right;
uniform float keyboard_height;
uniform vec2 resolution;

out vec3 n_color;
out vec2 v_texcoord;

out vec2 note_size;

void main() {
    float left = left * 2.0 - 1.0;
    float right = right * 2.0 - 1.0;
    float start = note_times.x * 2.0 - 1.0;
    float end = note_times.y * 2.0 - 1.0;
    //vec3 color = vec3(uvec3((clr & uint(0xFF)), (clr >> 8) & uint(0xFF), (clr >> 16) & uint(0xFF))) / 256.0;
    vec3 color = vec3(colors);
    vec3 bdr = color * 0.3;
    v_texcoord = texcoord;
    note_size = vec2(abs((right * 0.5 + 0.5) - (left * 0.5 + 0.5)), abs((end * 0.5 + 0.5) - (start * 0.5 + 0.5)));

    // n_type 0 = note border | n_type 1 = note color itself
    int n_type = gl_VertexID / 4;
    if (n_type == 0 || (n_type == 1 && note_size.y > 8.0 / resolution.y)) {
        if (int(gl_VertexID % 4) == 0) {
            gl_Position = vec4(left, start, 0.0, 1.0);
            n_color = color;
            if (n_type == 1) {
                gl_Position.xy += vec2(2.0 / resolution.x, 4.0 / resolution.y);
            }
        } else if (int(gl_VertexID % 4) == 1) {
            gl_Position = vec4(right, start, 0.0, 1.0);
            n_color = color * 0.7;
            if (n_type == 1) {
                gl_Position.xy += vec2(-2.0 / resolution.x, 4.0 / resolution.y);
            }
        } else if (int(gl_VertexID % 4) == 2) {
            gl_Position = vec4(right, end, 0.0, 1.0);
            n_color = color * 0.7;
            if (n_type == 1) {
                gl_Position.xy += vec2(-2.0 / resolution.x, -4.0 / resolution.y);
            }
        } else if (int(gl_VertexID % 4) == 3 ) {
            gl_Position = vec4(left, end, 0.0, 1.0);
            n_color = color;
            if (n_type == 1) {
                gl_Position.xy += vec2(2.0 / resolution.x, -4.0 / resolution.y);
            }
        }
    }
    if (n_type == 0) {
        n_color = bdr;
    }
    gl_Position.y = gl_Position.y * 0.5 + 0.5;
    gl_Position.y = gl_Position.y * (1.0 - keyboard_height) + keyboard_height;
    gl_Position.y = gl_Position.y * 2.0 - 1.0;
}"),
// n_fragment.glsl
String::from(
"#version 330

out vec4 note_color;

in vec3 n_color;
in vec2 v_texcoord;

uniform vec2 resolution;

in vec2 note_size;

void main() {
    vec3 col = n_color;
    vec2 texel_size = vec2(1.0 / resolution.x, 1.0 / resolution.y);
    vec2 mul = vec2(resolution.x / note_size.x,
                    resolution.y / note_size.y);
    note_color = vec4(col, 1.0);
}"),
// k_vertex.glsl
String::from(
"#version 330

uniform float left;
uniform float right;
uniform float keyboard_height;
uniform vec2 resolution;

uniform uint color;
uniform uint meta;
uniform float key_weight;

out vec3 k_color;
out float k_weight;
out vec2 v_texcoord;
out float k_pressed;
out float k_black;

void main() {
    float left = left * 2.0 - 1.0;
    float right = right * 2.0 - 1.0;

    uint clr = color;
    vec3 color = vec3(uvec3((clr & uint(0xFF)), (clr >> 8) & uint(0xFF), (clr >> 16) & uint(0xFF))) / 256.0;

    uint meta = meta;

    bool pressed = (meta & uint(2)) == uint(2);
    bool black = (meta & uint(1)) == uint(1);

    k_pressed = pressed ? 1.0 : 0.0;
    k_black = black ? 1.0 : 0.0;
    k_weight = key_weight;

    if (int(gl_VertexID % 4) == 0) {
        gl_Position = vec4(left, -1.0, 0.0, 1.0);
        v_texcoord = vec2(0.0, 0.0);
    } else if (int(gl_VertexID % 4) == 1) {
        gl_Position = vec4(right, -1.0, 0.0, 1.0);
        v_texcoord = vec2(1.0, 0.0);
    } else if (int(gl_VertexID % 4) == 2) {
        gl_Position = vec4(right, keyboard_height * 2.0 - 1.0, 0.0, 1.0);
        v_texcoord = vec2(1.0, 1.0);
    } else {
        gl_Position = vec4(left, keyboard_height * 2.0 - 1.0, 0.0, 1.0);
        v_texcoord = vec2(0.0, 1.0);
    }

    if (black && (gl_VertexID == 0 || gl_VertexID == 1)) {
        gl_Position.y += keyboard_height * 2.0/3.0;
    }

    if (pressed) {
        k_color = color;
        if ((gl_VertexID == 2 || gl_VertexID == 3) && !black) {
            k_color *= 0.6;
        } else if ((gl_VertexID == 0 || gl_VertexID == 1) && black) {
            k_color *= 0.3;
        }
    }
    else k_color = (black ? vec3(0.0) : vec3(1.0));
}"),
// k_fragment.glsl
String::from(
"#version 330

out vec4 key_color;

in vec3 k_color;
in float k_weight;
in vec2 v_texcoord;
in float k_pressed;
in float k_black;

void main() {
    key_color = vec4(k_color, 1.0);
    if (k_black < 0.5) {
        float shade_area = (k_pressed > 0.5) ? 0.05 * k_weight : 0.05;
        if (v_texcoord.y < shade_area) {
            key_color *= 0.5;
        }
        if (v_texcoord.x < 0.03 || v_texcoord.x > 0.97) {
            key_color *= 0.3;
        }
    } else {
        float shade_area = (k_pressed > 0.5) ? 0.03 * k_weight + 0.07 : 0.1;
        if (v_texcoord.y < shade_area || v_texcoord.x < 0.07 || v_texcoord.x > 0.93) {
            key_color += 0.2;
        }
    }
}"),
// b_vertex.glsl
String::from(
"#version 330

uniform float keyboard_height;
uniform vec3 color;

out vec3 b_color;
out vec2 b_texcoord;

void main() {
    vec2 pos;
    if (gl_VertexID % 4 == 0) {
        pos = vec2(0.0, keyboard_height);
        b_texcoord = vec2(0.0, 0.0);
    } else if (gl_VertexID % 4 == 1) {
        pos = vec2(1.0, keyboard_height);
        b_texcoord = vec2(1.0, 0.0);
    } else if (gl_VertexID % 4 == 2) {
        pos = vec2(1.0, keyboard_height * 1.05);
        b_texcoord = vec2(1.0, 1.0);
    } else {
        pos = vec2(0.0, keyboard_height * 1.05);
        b_texcoord = vec2(0.0, 1.0);
    }
    b_color = color;
    gl_Position = vec4(pos * 2.0 - 1.0, 0.0, 1.0);
}"),
// b_fragment.glsl
String::from(
"#version 330

in vec3 b_color;
in vec2 b_texcoord;

out vec4 bar_color;

void main() {
    bar_color = vec4(b_color * b_texcoord.y, 1.0);
}"),
    ];

    // iterate over file paths
    for (i, path) in file_paths.iter().enumerate() {
        let needs_recreation = needs_recreations[i];
        if needs_recreation {
            let mut f = File::create_new(
                absolute(path).unwrap()
            ).unwrap();
            f.write_all(shader_sources[i].as_bytes()).unwrap();
        }
    }
}