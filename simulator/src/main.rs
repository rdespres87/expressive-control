use expressive_core::midi::ToMidi;
use expressive_core::processor::{ControlProcessor, MpeInstrument};
use expressive_core::{Channel, InstrumentEvent, SensorEvent, U7};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
struct Scenario {
    events: Vec<SensorEvent>,
}

fn main() {
    let yaml_content = fs::read_to_string("simulator/scenario.yml")
        .expect("Failed to read simulator/scenario.yml");
    let scenario: Scenario = serde_yaml::from_str(&yaml_content).expect("Failed to parse YAML");

    let mut instrument = MpeInstrument::default();

    // We need a dummy event to initialize the array because it doesn't derive Default.
    let dummy_event = InstrumentEvent::NoteOff {
        channel: Channel::new(0),
        note: U7::new(0),
        velocity: U7::new(0),
    };
    let mut out_buffer = [dummy_event; 2];

    println!("Starting simulation from scenario.yml...\n");

    for event in scenario.events {
        println!("Input: {:?}", event);
        match instrument.process_event(event, &mut out_buffer) {
            Ok(count) => {
                for event_out in out_buffer.iter().take(count) {
                    let midi_msg = event_out.to_midi();
                    println!("  -> {:?} | Hex: {:02X?}", event_out, midi_msg.as_slice());
                }
            }
            Err(e) => {
                println!("  -> Error: {:?}", e);
            }
        }
        println!("---");
    }
}
