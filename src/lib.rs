#[macro_use]
extern crate vst;
extern crate time;
extern crate rand;
extern crate rand_distr;
extern crate ws;
extern crate webbrowser;
extern crate vst_gui;
extern crate smallvec;

use vst::buffer::AudioBuffer;
use vst::plugin::{Category, Info, Plugin, PluginParameters};
use vst::util::AtomicFloat;
use vst::api::{Events};
use vst::buffer::{ SendEventBuffer};
use vst::event::{Event, MidiEvent};
use vst::plugin::{CanDo, HostCallback,};
use std::sync::Arc;
use vst::host::{Host};
use smallvec::SmallVec;



/**
 * Midi
 */
fn get_note_name(midi_pitch: i32) -> String {
    let n = match midi_pitch % 12 {
        0 => "C",
        1 => "C#",
        2 => "D",
        3 => "D#",
        4 => "E",
        5 => "F",
        6 => "F#",
        7 => "G",
        8 => "G#",
        9 => "A",
        10 => "A#",
        11 => "B",
        _ => ""
    };

    let o = midi_pitch / 12 + 1;
    
    format!("{}-{}", n, o)
}


/**
 * Parameters
 */ 
struct UclidParameters {
    pulses: AtomicFloat,
    max_steps: AtomicFloat,
    velocity: AtomicFloat,
    note: AtomicFloat,
    offset: AtomicFloat,
    note_length: AtomicFloat,
    multiplier: AtomicFloat
}


impl Default for UclidParameters {
    fn default() -> UclidParameters {
        UclidParameters {
            // 4/4
            pulses: AtomicFloat::new(0.125),
            max_steps: AtomicFloat::new(0.125),
            velocity: AtomicFloat::new(0.5),
            note: AtomicFloat::new(0.5),
            offset: AtomicFloat::new(0.),
            note_length: AtomicFloat::new(0.5),
            multiplier: AtomicFloat::new(0.25)
        }
    }
}


static MAX_STEPS: i32 = 32;


impl PluginParameters for UclidParameters {
    // the `get_parameter` function reads the value of a parameter.
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.pulses.get(),
            1 => self.max_steps.get(),
            2 => self.multiplier.get(),
            3 => self.offset.get(),
            4 => self.note.get(),
            5 => self.velocity.get(),
            6 => self.note_length.get(),
            _ => 0.0,
        }
    }

    // the `set_parameter` function sets the value of a parameter.
    fn set_parameter(&self, index: i32, val: f32) {        
        #[allow(clippy::single_match)]
        match index {
            0 => self.pulses.set(val.max(0.03125)),
            1 => self.max_steps.set(val.max(0.03125)),
            2 => self.multiplier.set(val.max(0.125)),
            3 => self.offset.set(val),
            4 => self.note.set(val),
            5 => self.velocity.set(val),
            6 => self.note_length.set(val),
            _ => (),
        }
    }

    // This is what will display underneath our control.  We can
    // format it into a string that makes the most since.
    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 =>  format!("{:.0}", (self.pulses.get() * MAX_STEPS as f32).floor()),
            1 =>  format!("{:.0}", (self.max_steps.get() * MAX_STEPS as f32).floor()),
            2 =>  format!("{:.0}", (self.multiplier.get() * 4.).floor()),
            3 =>  format!("{:.0}", self.offset.get() * self.max_steps.get() * MAX_STEPS as f32),
            4 =>  get_note_name((self.note.get() * 127.) as i32),
            5 =>  format!("{:.0}", self.velocity.get() * 127.),
            6 =>  format!("{:.0}", self.note_length.get() * 3.),
            _ => "".to_string()
        }
    }
    
    
    // This shows the control's name.
    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "Pulses",
            1 => "Total steps",
            2 => "Multiplier",
            3 => "Offset",
            4 => "Note",
            5 => "Velocity",
            6 => "Note length",
            _ => "",
        }
        .to_string()
    }
}









/**
 * Plugin
 */ 

#[derive(Default)]
struct Uclid {
    host: HostCallback,
    sample_rate: f32,
    noteoff_events: Vec<DelayedMidiEvent>,
    send_buffer: SendEventBuffer,
    events: Vec<MidiEvent>,
    params: Arc<UclidParameters>,
    bpm: f32,
    last_note: f64
}

struct DelayedMidiEvent {
    event: MidiEvent,
    time_left: f64
}


fn euclidian_rythm(steps: usize, pulses: usize) -> Result<SmallVec::<[u8; 64]>, &'static str> {
    let mut pattern = SmallVec::with_capacity(pulses);
    pattern.clear();

    if pulses > steps {
        return Err("more pulses than steps.");
    }

    let mut divisor = steps - pulses;

    let mut level = 0;
    let mut counts = SmallVec::<[usize; 64]>::new();
    let mut remainders = SmallVec::<[usize; 64]>::new();

    remainders.push(pulses);

    // Run the euclid algorithm, store all the intermediate results
    loop {
        counts.push(divisor / remainders[level]);
        let r = remainders[level];
        remainders.push(divisor % r);

        divisor = remainders[level];
        level += 1;

        if remainders[level] <= 1 {
            break;
        }
    }
    counts.push(divisor);

    // Build the pattern
    fn build(
        counts: &[usize],
        pattern: &mut SmallVec<[u8; 64]>,
        remainders: &[usize],
        level: isize,
    ) {
        if level == -1 {
            pattern.push(0);
        } else if level == -2 {
            pattern.push(1);
        } else {
            for _ in 0..counts[level as usize] {
                build(counts, pattern, remainders, level - 1);
            }
            if remainders[level as usize] != 0 {
                build(counts, pattern, remainders, level - 2);
            }
        }
    }

    build(
        counts.as_slice(),
        &mut pattern,
        remainders.as_slice(),
        level as isize,
    );

    // Put a 1 on the first step
    let index_first_one = pattern.iter().position(|&x| x == 1).unwrap();
    pattern.rotate_left(index_first_one);

    // println!("{:?}", pattern);

    Ok(pattern)
}


impl Uclid {
    fn do_rhythm(&mut self, pattern: &SmallVec<[u8;64]>) {
        // get params
        let max_steps = (self.params.max_steps.get() * MAX_STEPS as f32).floor();
        let velocity = (self.params.velocity.get() * 127.) as u8; 
        let nooote = (self.params.note.get() * 127.) as u8; 
        let note_length = self.params.note_length.get() * 3.; 
        let multiplier = (self.params.multiplier.get() * 4.).floor() as f64; 
        
        let time_info = self.host.get_time_info(1 << 9).unwrap();
        
        let offset = self.params.offset.get() * max_steps;
        
        
        let note = ((time_info.ppq_pos * multiplier).floor() + offset as f64) % max_steps as f64;
        

        if self.last_note != note { 
            self.last_note = note;
            let pattern_note = pattern[note as usize];

            if pattern_note == 1 {
                self.send_buffer.send_events(vec![
                    MidiEvent {
                        data: [144, nooote, velocity],
                        delta_frames: 0,
                        live: true,
                        note_length: None,
                        note_offset: None,
                        detune: 0,
                        note_off_velocity: 0,
                    },
                ], &mut self.host);
    
                self.noteoff_events.push(DelayedMidiEvent {
                    time_left: note_length as f64,
                    event: MidiEvent {
                        data: [128, nooote, velocity],
                        delta_frames: 0,
                        live: true,
                        note_length: None,
                        note_offset: None,
                        detune: 0,
                        note_off_velocity: 0,
                    }
                })
            }
        }


        for event in &mut self.noteoff_events {
            if event.time_left <= 0.0 {
                self.send_buffer.send_events(vec![event.event], &mut self.host);
            }
        }

        self.noteoff_events.retain(|e| e.time_left > 0.0)
        


        // Immediate
        // self.immediate_events.clear();
    }
}


impl Plugin for Uclid {
    fn new(host: HostCallback) -> Self {
        let mut p = Uclid::default();
        p.host = host;
        p.params = Arc::new(UclidParameters::default());
        p.bpm = 120.0;
        
        p
    }

    fn get_info(&self) -> Info {
        Info {
            name: "Uclid".to_string(),
            vendor: "Rein van der Woerd".to_string(),
            unique_id: 298467,
            version: 1,
            inputs: 2,
            outputs: 2,
            parameters: 7,
            category: Category::Effect,
            ..Default::default()
        }
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
    }

    fn process_events(&mut self, events: &Events) {
        for e in events.events() {
            #[allow(clippy::single_match)]
            match e {
                Event::Midi(e) => self.events.push(e),
                _ => (),
            }
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        let max_steps = (self.params.max_steps.get() * MAX_STEPS as f32).floor() as usize;
        let pulses = (self.params.pulses.get() * MAX_STEPS as f32).floor() as usize; // not more pulses than steps

        let pattern = euclidian_rythm(max_steps, if pulses > max_steps { max_steps} else {pulses} ).unwrap();

        for (input, output) in buffer.zip() {
            for (in_sample, out_sample) in input.iter().zip(output) {
                *out_sample = *in_sample;

                self.do_rhythm(&pattern);


                for mut event in &mut self.noteoff_events {
                    event.time_left -= 1. / self.sample_rate as f64;
                }
            }
        }

        // Forward all midi events
        self.send_buffer.send_events(&self.events, &mut self.host);
        self.events.clear();
    }

    fn can_do(&self, can_do: CanDo) -> vst::api::Supported {
        use vst::api::Supported::*;
        use vst::plugin::CanDo::*;

        match can_do {
            SendEvents | SendMidiEvent | ReceiveEvents | ReceiveMidiEvent => Yes,
            _ => No,
        }
    }
    
    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }
}




// This part is important!  Without it, our plugin won't work.
plugin_main!(Uclid);