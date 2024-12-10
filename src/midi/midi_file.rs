use std::fs::File;
use std::io::Seek;
use std::sync::{Arc, Mutex};
use rayon::prelude::*;

use super::byte_reader;
use crate::midi::midi_track_parser::MIDIEvent;
use crate::util::iter_ext::{merge_midi_events, merge_notes, merge_tempo_evs};

use super::midi_track_parser::{MIDITrack, TempoEvent, Note};

pub struct TrackPointer {
    pub start: u64,
    pub len: u32
}

pub struct MIDIFile {
    pub ppq: u16,
    pub trk_count: u16,
    pub track_locations: Vec<TrackPointer>,
    pub tracks: Vec<MIDITrack>,
    pub note_counts: Vec<u64>,

    tempo_evs: Vec<TempoEvent>
}

impl MIDIFile {
    pub fn new(path: String) -> Result<Self,()> {
        let file_stream = Arc::new(Mutex::new(
            File::open(path).unwrap()
        ));

        let mut s = Self {
            ppq: 0,
            trk_count: 0,
            track_locations: Vec::new(),
            tracks: Vec::new(),
            note_counts: Vec::new(),

            tempo_evs: Vec::new()
        };

        {
            let mut fs = file_stream.lock().unwrap();
            s.parse_header(&mut fs).unwrap();
            s.populate_track_locations(&mut fs).unwrap();
        }
        
        let s_box = Box::new(&mut s);

        s_box.tracks = Vec::with_capacity(s_box.trk_count as usize);

        for i in 0usize..(s_box.trk_count as usize) {
            s_box.tracks.push(MIDITrack::new(i, s_box.ppq, Arc::clone(&file_stream), &s_box.track_locations[i]).unwrap());
        }

        let tempo_evs_seq: Vec<Vec<TempoEvent>>;
        println!("----- Parse pass 1 -----");
        {
            (s_box.note_counts, tempo_evs_seq) = s_box.tracks.par_iter_mut().enumerate().map(|(i, track)| {
                while !track.ended {
                    track.parse_ev().unwrap();
                }
                println!("track {} of {} parsed", i, &s_box.trk_count);
                track.prep_for_pass_two().unwrap();
                (track.note_count, std::mem::take(&mut track.tempo_evs))
            }).collect();
        }

        s_box.tempo_evs = merge_tempo_evs(tempo_evs_seq);

        // merge tempo events lol
        //let t_evs: Vec<TempoEvent> = merge_tempo_evs(tempo_evs_seq);

        /*println!("----- Parse pass 2 -----");
        {
            s_box.midi_evs = merge_midi_events(s_box.tracks.par_iter_mut().enumerate().map(|(i, track)| {
                while !track.ended {
                    track.parse_pass_two(&t_evs).unwrap();
                }
                println!("track {} of {} parsed", i, &s_box.trk_count);
                track.midi_evs.iter().collect()
            }).collect());
        };*/

        Ok(s)
    }

    // move from self to Vec<MIDIEvent>
    pub fn get_evs_and_notes(self) -> (Vec<MIDIEvent>, Vec<Vec<Note>>) {
        println!("----- Getting events (Parse pass 2) -----");
        let (evs, mut notes): (Vec<Vec<MIDIEvent>>, Vec<Vec<Vec<Note>>>) = self.tracks.into_par_iter().enumerate().map(|(i, mut track)| {
            while !track.ended {
                track.parse_pass_two(&self.tempo_evs).unwrap();
            }
            println!("track {} of {} parsed", i, &self.trk_count);
            (track.midi_evs,
             track.notes)

        }).collect();
        let mut merged_notes_at_keys: Vec<Vec<Note>> = Vec::new();
        for _ in 0..128 {
            merged_notes_at_keys.push(
                merge_notes(notes.iter_mut().map(|n| n.pop().unwrap()).collect::<Vec<_>>())
            );
        }
        println!("{}", merged_notes_at_keys[0].len());
        println!("merging events...");
        (merge_midi_events(evs),
         merged_notes_at_keys)
    }

    fn parse_header(&mut self, stream: &mut File) -> Result<(),&str> {
        // assuming header length in total is 14
        // MThd header
        let mthd: u32 = byte_reader::read_u32(stream).unwrap();
        assert_eq!(mthd, 0x4D546864);

        // length
        let h_len: u32 = byte_reader::read_u32(stream).unwrap();
        assert_eq!(h_len, 6);
        // format lol
        let m_fmt: u16 = byte_reader::read_u16(stream).unwrap();
        if m_fmt == 2 {
            return Err("please stop using format 2")
        }
        // track count (i think)
        let m_trk_count: u16 = byte_reader::read_u16(stream).unwrap();
        let m_ppq: u16 = byte_reader::read_u16(stream).unwrap();
        
        self.trk_count = m_trk_count;
        self.ppq = m_ppq;

        Ok(())
    }

    fn populate_track_locations(&mut self, stream: &mut File) -> Result<(), &str> {
        for _ in 0..self.trk_count {
            let mtrk: u32 = byte_reader::read_u32(stream).unwrap();
            assert_eq!(mtrk, 0x4D54726B);
            
            let t_len: u32 = byte_reader::read_u32(stream).unwrap();
            let pos: u64 = stream.stream_position().unwrap();

            stream.seek_relative(t_len as i64).unwrap();

            self.track_locations.push(TrackPointer {
                start: pos,
                len: t_len
            });
        }

        Ok(())
    }
}