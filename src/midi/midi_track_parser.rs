use std::fs::File;
use std::sync::{Arc, Mutex};

use crate::midi::buffered_byte_reader::BufferedByteReader;
use crate::midi::midi_file::TrackPointer;

pub struct TempoEvent {
    pub time: u64, // absolute time
    pub time_norm: f32,
    pub tempo: u32
}

pub enum MetaEventName {
    Marker=0x06,
    TimeSignature=0x58,
    KeySignature=0x59
}

pub struct MetaEvent {
    pub time: f32,
    pub meta_name: MetaEventName,
    pub data: Vec<u8>
}

//#[derive(PartialEq, Eq)]
pub enum MIDIEventType {
    NoteOff=0x80,
    NoteOn=0x90,
    ControlEvent=0xB0,
    PitchBend=0xE0,
}

//#[derive(PartialEq, Eq)]
pub struct MIDIEvent {
    pub time: f32, // relative time
    pub command: MIDIEventType,
    pub data: Vec<u8>
}

struct UnendedNote {
    pub id: i32,
    pub vel: u8
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Note {
    pub start: u32,
    pub end: u32,
    pub channel: u8,
    pub track: usize,
    pub velocity: u8,
}

pub struct MIDITrack {
    pub rdr: BufferedByteReader,
    pub ev_count: u64,
    pub tempo_ev_count: u64,
    pub note_count: u64,
    pub ended: bool,
    prev_cmd: u8,

    pub tempo_evs: Vec<TempoEvent>,
    pub midi_evs: Vec<MIDIEvent>,
    pub meta_evs: Vec<MetaEvent>,
    pub notes: Vec<Vec<Note>>,
    note_counts: [usize; 256],
    pub track_len: u64,
    pub track_len_p2: f64,
    pub t_track_time: f64,
    pub tempo_id: usize,
    pub tempo_multi: f64,

    unended_notes: Vec<Vec<UnendedNote>>,
    unended_init: bool,
    curr_note_idx: [usize; 256],

    valid_delta: f64, // to add delta times of skipped / unneeded events lol
    ppq: u16,
    track_num: usize,

    tick_based_parsing: bool,
    pub key_range: [u8; 2]
}

impl MIDITrack {
    pub fn new(t_num: usize, ppq: u16, stream: Arc<Mutex<File>>, loc: &TrackPointer, tick_based_parsing: bool) -> Result<Self, ()> {
        let mt = Self {
            rdr: BufferedByteReader::new(stream, loc.start as usize, loc.len as usize, 100000).unwrap(),
            ev_count: 0,
            tempo_ev_count: 0,
            note_count: 0,
            ended: false,
            prev_cmd: 0x00,

            tempo_evs: Vec::new(),
            midi_evs: Vec::new(),
            meta_evs: Vec::new(),
            notes: Vec::new(),
            note_counts: [0usize; 256],
            track_len: 0,
            track_len_p2: 0.0f64,
            t_track_time: 0.0f64,
            tempo_id: 0usize,
            tempo_multi: (500000.0 / ppq as f64) / 1000000.0,

            unended_notes: Vec::new(),
            unended_init: false,
            curr_note_idx: [0usize; 256],

            valid_delta: 0.0f64,
            ppq,
            track_num: t_num,

            tick_based_parsing,
            key_range: [255, 0]
        };
        Ok(mt)
    }

    fn read_delta(&mut self) -> u64 {
        let mut n: u64 = 0;
        loop {
            let b = self.rdr.read_byte().unwrap();
            n = (n << 7) | ((b & 0x7F) as u64);
            if (b & 0x80) == 0x00 { break; }
        }
        n
    }

    fn read_delta_time(&mut self, t_evs: &Vec<TempoEvent>) -> f64 {
        let mut n: u64 = 0;
        loop {
            let b = self.rdr.read_byte().unwrap();
            n = (n << 7) | ((b & 0x7F) as u64);
            if (b & 0x80) == 0x00 { break; }
        }
        self.track_len_p2 += n as f64;

        if self.tempo_id < t_evs.len() && self.track_len_p2 > t_evs[self.tempo_id].time as f64 {
            let mut t: i64 = (self.track_len_p2 - n as f64) as i64;
            let mut v: f64 = 0.0;
            while self.tempo_id < t_evs.len() && self.track_len_p2 > t_evs[self.tempo_id].time as f64 {
                v += ((t_evs[self.tempo_id].time as i64 - t) as f64) * self.tempo_multi;
                t = t_evs[self.tempo_id].time as i64;
                self.tempo_multi = (t_evs[self.tempo_id].tempo as f64 / self.ppq as f64) / 1000000.0;
                self.tempo_id += 1;
            }
            v += (self.track_len_p2 - t as f64) * self.tempo_multi;
            return v;
        } else {
            return (n as f64) * self.tempo_multi;
        }

    }

    pub fn parse_ev(&mut self) -> Result<(), ()> {
        if self.ended { 
            return Ok(())
        }
        let delta = self.read_delta();
        self.track_len += delta;

        let mut command: u8 = self.rdr.read_byte().unwrap();
        if command < 0x80 {
            self.rdr.seek(-1, 1).unwrap();
            command = self.prev_cmd;
        }

        self.prev_cmd = command;

        match command & 0xF0 {
            0x80 => {
                self.rdr.skip_bytes(2)?;
            },
            0x90 => {
                let key = self.rdr.read_byte()?;
                if key <= self.key_range[0] {
                    self.key_range[0] = key;
                }
                if key >= self.key_range[1] {
                    self.key_range[1] = key;
                }

                if self.rdr.read_byte()? > 0 { 
                    self.note_count += 1;
                    self.note_counts[key as usize] += 1;
                }
            },
            0xA0 | 0xB0 | 0xE0 => {
                self.rdr.skip_bytes(2)?;
            },
            0xC0 | 0xD0 => {
                self.rdr.skip_bytes(1)?;
            },
            0xF0 => {
                match command {
                    0xFF => {
                        let cmd2: u8 = self.rdr.read_byte()?;
                        let val = self.read_delta() as usize;
                        
                        match cmd2 {
                            0x00 => { self.rdr.skip_bytes(2)?; }
                            0x01..=0x07 | 0x0A => {
                                self.rdr.skip_bytes(val)?;
                            }
                            0x7F => { self.rdr.skip_bytes(val)?; }
                            0x20 => { self.rdr.skip_bytes(1)?; }
                            0x21 => { self.rdr.skip_bytes(1)?; }
                            0x2F => { self.ended = true; }
                            0x51 => {
                                let mut tempo: u32 = 0;
                                for _ in 0..3 {
                                    tempo = (tempo << 8) | (self.rdr.read_byte()? as u32);
                                }

                                self.tempo_evs.push(
                                    TempoEvent {
                                        time: self.track_len,
                                        time_norm: 0.0,
                                        tempo
                                    }
                                );
                                self.tempo_ev_count += 1;
                            }
                            0x54 => { self.rdr.skip_bytes(5)?; }
                            0x58 => { self.rdr.skip_bytes(4)?; }
                            0x59 => { self.rdr.skip_bytes(2)?; }
                            _ => {
                                println!("unknown sys ev {}", cmd2);
                                self.rdr.skip_bytes(val)?;
                                self.ev_count -= 1;
                            }
                        };
                    }
                    0xF0 => {
                        let sysex_len = self.read_delta();
                        self.rdr.skip_bytes(sysex_len as usize)?;
                    }
                    0xF2 => {
                        self.rdr.skip_bytes(2)?;
                    }
                    0xF3 => {
                        self.rdr.skip_bytes(1)?;
                    },
                    0xF7 => {
                        let sysex_len = self.read_delta();
                        self.rdr.skip_bytes(sysex_len as usize)?;
                    }
                    _ => {}
                }
            },
            _ => {}
        }
        self.ev_count += 1;
        Ok(())
    }

    pub fn prep_for_pass_two(&mut self) -> Result<(),()> {
        //reset rdr i think
        self.rdr.seek(0, 0).unwrap();
        self.prev_cmd = 0x00;
        self.ended = false;

        for _ in 0..256 {
            self.notes.push(Vec::new());
        }

        Ok(())
    }

    pub fn parse_pass_two(&mut self, t_evs: &Vec<TempoEvent>) -> Result<(),()> {
        if self.ended {
            return Ok(())
        }

        if !self.unended_init {
            for _ in 0..256*16 {
                self.unended_notes.push(Vec::new());
            }
            self.unended_init = true;
        }

        let delta = self.read_delta_time(t_evs);
        self.valid_delta += delta;
        self.t_track_time += delta;
        let mut command: u8 = self.rdr.read_byte().unwrap();
        if command < 0x80 {
            self.rdr.seek(-1, 1).unwrap();
            command = self.prev_cmd;
        }

        self.prev_cmd = command;

        let c: u8 = command & 0xF0;
        let ch: u8 = command & 0x0F;
        match c {
            0x80 => {
                //self.rdr.skip_bytes(2)?;
                let key = self.rdr.read_byte()?;
                let mut vel = self.rdr.read_byte()?;

                let un = &mut self.unended_notes[key as usize * 16 + ch as usize];
                if un.len() != 0 {
                    let n = un.pop().unwrap();
                    if n.id != -1 {
                        self.notes[key as usize][n.id as usize].end = if self.tick_based_parsing {
                            self.track_len_p2 as u32
                        } else {
                            (self.t_track_time * 1000000.0) as u32
                        };
                        self.notes[key as usize][n.id as usize].velocity = n.vel;
                        vel = n.vel;
                    }
                }

                self.midi_evs.push(
                    MIDIEvent {
                        time: self.t_track_time as f32,
                        command: MIDIEventType::NoteOff,
                        data: vec![ch, key, vel]
                    }
                );
                self.valid_delta = 0.0;
            },
            0x90 => {
                let key = self.rdr.read_byte()?;
                let vel = self.rdr.read_byte()?;
                self.midi_evs.push(
                    MIDIEvent {
                        time: self.t_track_time as f32,
                        command: if vel > 0 { MIDIEventType::NoteOn } else { MIDIEventType::NoteOff },
                        data: vec![ch, key, vel]
                    }
                );

                if vel == 0 {
                    let un = &mut self.unended_notes[key as usize * 16 + ch as usize];
                    if un.len() != 0 {
                        let n = un.pop().unwrap();
                        if n.id != -1 {
                            self.notes[key as usize][n.id as usize].end = if self.tick_based_parsing {
                                self.track_len_p2 as u32
                            } else {
                                (self.t_track_time * 1000000.0) as u32
                            };
                            self.notes[key as usize][n.id as usize].velocity = n.vel;
                            
                        }
                    }
                } else {
                    self.unended_notes[key as usize * 16 + ch as usize].push(UnendedNote {
                        id: self.curr_note_idx[key as usize] as i32,
                        vel
                    });
                    /*self.notes[key as usize][self.curr_note_idx[key as usize]] = Note {
                        start: (self.t_track_time * 1000000.0) as u64,
                        end: 1000000000000,
                        channel: ch
                    };*/
                    //self.notes[key as usize][self.curr_note_idx[key as usize]].start = (self.t_track_time * 1000000.0) as u64;
                    //self.notes[key as usize][self.curr_note_idx[key as usize]].channel = ch;
                    self.notes[key as usize].push(Note {
                        start: if self.tick_based_parsing {
                            self.track_len_p2 as u32
                        } else {
                            (self.t_track_time * 1000000.0) as u32
                        },
                        end: 10000000,
                        channel: ch,
                        track: self.track_num,
                        velocity: 0
                    });
                    self.curr_note_idx[key as usize] += 1;
                }

                self.valid_delta = 0.0;
            },
            0xB0 => {
                let ctrl_num = self.rdr.read_byte()?;
                let ctrl_val = self.rdr.read_byte()?;
                self.midi_evs.push(MIDIEvent {
                    time: self.t_track_time as f32,
                    command: MIDIEventType::ControlEvent,
                    data: vec![ch, ctrl_num, ctrl_val]
                });
                
                self.valid_delta = 0.0;
            },
            0xE0 => {
                let v1 = self.rdr.read_byte()?;
                let v2 = self.rdr.read_byte()?;
                self.midi_evs.push(MIDIEvent {
                    time: self.t_track_time as f32,
                    command: MIDIEventType::PitchBend,
                    data: vec![ch, v1, v2]
                });
                
                self.valid_delta = 0.0;
            },
            0xA0 => {
               self.rdr.skip_bytes(2)?;
            },
            0xC0 | 0xD0 => {
                self.rdr.skip_bytes(1)?;
            },
            0xF0 => {
                match command {
                    0xFF => {
                        let cmd2: u8 = self.rdr.read_byte()?;
                        let val = self.read_delta() as usize;
                        
                        match cmd2 {
                            0x00 => { self.rdr.skip_bytes(2)?; }
                            // marker
                            0x06 => {
                                let mut text_bytes: Vec<u8> = vec![0u8; val];
                                self.rdr.read(&mut text_bytes[0..val], val).unwrap();
                                self.meta_evs.push(MetaEvent {
                                    time: self.t_track_time as f32,
                                    meta_name: MetaEventName::Marker,
                                    data: text_bytes
                                });
                            }
                            0x01..=0x05 | 0x07 | 0x0A => {
                                self.rdr.skip_bytes(val)?;
                            }
                            0x7F => { self.rdr.skip_bytes(val)?; }
                            0x20 => { self.rdr.skip_bytes(1)?; }
                            0x21 => { self.rdr.skip_bytes(1)?; }
                            0x2F => { self.ended = true; }
                            0x51 => {
                                let mut tempo: u32 = 0;
                                for _ in 0..3 {
                                    tempo = (tempo << 8) | (self.rdr.read_byte()? as u32);
                                }

                                self.tempo_evs.push(TempoEvent {
                                    time: self.track_len_p2 as u64,
                                    time_norm: self.t_track_time as f32,
                                    tempo
                                });
                            }
                            0x54 => { self.rdr.skip_bytes(5)?; }
                            0x58 => { self.rdr.skip_bytes(4)?; }
                            0x59 => { self.rdr.skip_bytes(2)?; }
                            _ => {
                                println!("unknown sys ev {}", cmd2);
                                self.rdr.skip_bytes(val)?;
                            }
                        };
                    }
                    0xF0 => {
                        let sysex_len = self.read_delta();
                        self.rdr.skip_bytes(sysex_len as usize)?;
                    }
                    0xF2 => {
                        self.rdr.skip_bytes(2)?;
                    }
                    0xF3 => {
                        self.rdr.skip_bytes(1)?;
                    },
                    0xF7 => {
                        let sysex_len = self.read_delta();
                        self.rdr.skip_bytes(sysex_len as usize)?;
                    }
                    _ => {}
                }
            },
            _ => {}
        }
        Ok(())
    }
}