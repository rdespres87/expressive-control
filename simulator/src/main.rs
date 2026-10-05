use expressive_core::midi::ToMidi;
use expressive_core::processor::{ControlProcessor, MpeInstrument};
use expressive_core::{Channel, InstrumentEvent, SensorEvent, U7};
#[cfg(unix)]
use midir::os::unix::VirtualOutput;
use midir::MidiOutput;
use serde::{Deserialize, Serialize};
use std::fs;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct SimulatorAction {
    event: Option<SensorEvent>,
    wait_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Scenario {
    actions: Vec<SimulatorAction>,
}

fn main() {
    // 1. Initialize MIDI Output
    let midi_out = MidiOutput::new("Expressive Simulator").expect("Failed to create MIDI output");
    let mut conn_out = midi_out
        .create_virtual("Expressive MPE Out")
        .expect("Failed to create virtual MIDI port");

    println!("🎵 Virtual MIDI port 'Expressive MPE Out' created!");
    println!("👉 Please open your DAW (Ableton, Logic) or a Synth and listen to this port.");
    println!("⏳ Starting simulation in 5 seconds...\n"); // Reduced to 5s for convenience
    sleep(Duration::from_secs(5));

    // 2. Load Scenario
    let yaml_content = fs::read_to_string("simulator/scenario.yml")
        .expect("Failed to read simulator/scenario.yml");
    let scenario: Scenario = serde_yaml::from_str(&yaml_content).expect("Failed to parse YAML");

    let mut instrument = MpeInstrument::default();
    let dummy_event = InstrumentEvent::NoteOff {
        channel: Channel::new(0),
        note: U7::new(0),
        velocity: U7::new(0),
    };
    let mut out_buffer = [dummy_event; 2];

    // 3. Play Scenario
    for action in scenario.actions {
        if let Some(event) = action.event {
            println!("Input: {:?}", event);
            match instrument.process_event(event, &mut out_buffer) {
                Ok(count) => {
                    for event_out in out_buffer.iter().take(count) {
                        let midi_msg = event_out.to_midi();
                        println!("  -> {:?} | Hex: {:02X?}", event_out, midi_msg.as_slice());
                        if let Err(e) = conn_out.send(midi_msg.as_slice()) {
                            println!("  -> Error sending MIDI: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("  -> Error: {:?}", e);
                }
            }
        }

        if let Some(ms) = action.wait_ms {
            sleep(Duration::from_millis(ms));
        }
    }

    println!("✅ Simulation complete. Closing port.");
}
