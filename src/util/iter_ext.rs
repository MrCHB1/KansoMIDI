use std::{cmp::Reverse, collections::BinaryHeap};

use crate::midi::midi_track_parser::{MIDIEvent, Note, TempoEvent};

impl PartialOrd for TempoEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TempoEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}

/*impl PartialOrd for MIDIEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MIDIEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}*/

pub fn merge_two_tempo_seqs(seq1: Vec<TempoEvent>, seq2: Vec<TempoEvent>) -> Vec<TempoEvent> {
    let mut enum1 = seq1.into_iter();
    let mut enum2 = seq2.into_iter();
    let mut e1 = enum1.next();
    let mut e2 = enum2.next();
    let mut res = Vec::new();

    loop {
        match e1 {
            Some(ref en1) => {
                match e2 {
                    Some(ref en2) => {
                        if en1.time < en2.time {
                            res.push(e1.unwrap());
                            e1 = enum1.next();
                        } else {
                            res.push(e2.unwrap());
                            e2 = enum2.next();
                        }
                    }
                    None => {
                        res.push(e1.unwrap());
                        e1 = enum1.next();
                    }
                }
            },
            None => {
                if e2 == None { break; }
                else {
                    res.push(e2.unwrap());
                    e2 = enum2.next();
                }
            }
        }
    }

    res
}

pub fn merge_tempo_evs(seq: Vec<Vec<TempoEvent>>) -> Vec<TempoEvent> {
    let mut b1 = seq.into_iter().collect::<Vec<_>>();
    let mut b2 = Vec::new();
    if b1.len() == 0 {
        return Vec::new();
    }
    while b1.len() > 1 {
        while b1.len() > 0 {
            if b1.len() == 1 {
                b2.push(b1.remove(0));
            } else {
                b2.push(merge_two_tempo_seqs(b1.remove(0), b1.remove(0)));
            }
        }
        b1 = b2;
        b2 = Vec::new();
    }
    b1.remove(0)
}

pub fn merge_two_note_seqs(seq1: Vec<Note>, seq2: Vec<Note>) -> Vec<Note> {
    let mut enum1 = seq1.into_iter();
    let mut enum2 = seq2.into_iter();
    let mut e1 = enum1.next();
    let mut e2 = enum2.next();
    let mut res = Vec::new();

    loop {
        match e1 {
            Some(ref en1) => {
                match e2 {
                    Some(ref en2) => {
                        if en1.start < en2.start || (en1.start == en2.start && en1.track < en2.track) {
                            res.push(e1.unwrap());
                            e1 = enum1.next();
                        } else {
                            res.push(e2.unwrap());
                            e2 = enum2.next();
                        }
                    }
                    None => {
                        res.push(e1.unwrap());
                        e1 = enum1.next();
                    }
                }
            },
            None => {
                if e2 == None { break; }
                else {
                    res.push(e2.unwrap());
                    e2 = enum2.next();
                }
            }
        }
    }

    res
}

pub fn merge_notes(seq: Vec<Vec<Note>>) -> Vec<Note> {
    let mut b1 = seq.into_iter().collect::<Vec<_>>();
    let mut b2 = Vec::new();
    if b1.len() == 0 {
        return Vec::new();
    }
    while b1.len() > 1 {
        while b1.len() > 0 {
            if b1.len() == 1 {
                b2.push(b1.remove(0));
            } else {
                b2.push(merge_two_note_seqs(b1.remove(0), b1.remove(0)));
            }
        }
        b1 = b2;
        b2 = Vec::new();
    }
    b1.remove(0)
}

pub fn merge_two_seqs(seq1: Vec<MIDIEvent>, seq2: Vec<MIDIEvent>) -> Vec<MIDIEvent> {
    let mut enum1= seq1.into_iter();
    let mut enum2 = seq2.into_iter();
    let mut e1 = enum1.next();
    let mut e2 = enum2.next();
    let mut res = Vec::new();

    loop {
        match e1 {
            Some(ref mut en1) => {
                match e2 {
                    Some(ref mut en2) => {
                        if en1.time <= en2.time {
                            //en2.time -= en1.time;
                            res.push(e1.unwrap());
                            e1 = enum1.next();
                        } else {
                            //en1.time -= en2.time;
                            res.push(e2.unwrap());
                            e2 = enum2.next();
                        }
                    }
                    None => {
                        res.push(e1.unwrap());
                        e1 = enum1.next();
                    }
                }
            }
            None => {
                if e2.is_none() { break; }
                else {
                    res.push(e2.unwrap());
                    e2 = enum2.next();
                }
            }
        }
    }

    res
}

pub fn merge_midi_events(seq: Vec<Vec<MIDIEvent>>) -> Vec<MIDIEvent> {
    let mut b1 = seq.into_iter().collect::<Vec<_>>();
    let mut b2 = Vec::new();
    if b1.len() == 0 {
        return Vec::new();
    }
    while b1.len() > 1 {
        while b1.len() > 0 {
            if b1.len() == 1 {
                b2.push(b1.remove(0));
            } else {
                b2.push(merge_two_seqs(b1.remove(0), b1.remove(0)));
            }
        }
        b1 = b2;
        b2 = Vec::new();
    }
    b1.remove(0)
}