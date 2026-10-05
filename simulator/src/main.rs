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
    let args: Vec<String> = std::env::args().collect();
    let is_hil = args.len() > 1;

    let yaml_content = fs::read_to_string("simulator/scenario.yml")
        .expect("Failed to read simulator/scenario.yml");
    let scenario: Scenario = serde_yaml::from_str(&yaml_content).expect("Failed to parse YAML");

    if is_hil {
        // HIL MODE
        let port_name = &args[1];
        println!("🎵 HIL Mode: Opening serial port {}...", port_name);
        let mut port = serialport::new(port_name, 115200)
            .timeout(Duration::from_millis(100))
            .open()
            .expect("Failed to open serial port");

        println!("⏳ Starting HIL simulation in 3 seconds...\n");
        sleep(Duration::from_secs(3));

        for action in scenario.actions {
            if let Some(event) = action.event {
                println!("Injecting: {:?}", event);
                let mut buf = [0u8; 32];
                let serialized =
                    postcard::to_slice_cobs(&event, &mut buf).expect("Failed to serialize");

                if let Err(e) = port.write_all(serialized) {
                    println!("  -> Error sending to Serial: {}", e);
                }
            }

            if let Some(ms) = action.wait_ms {
                sleep(Duration::from_millis(ms));
            }
        }
        println!("✅ HIL Simulation complete.");
    } else {
        // PC SIMULATION MODE
        println!("💻 PC Simulation Mode");
        let midi_out =
            MidiOutput::new("Expressive Simulator").expect("Failed to create MIDI output");

        #[cfg(unix)]
        let mut conn_out = midi_out
            .create_virtual("Expressive MPE Out")
            .expect("Failed to create virtual MIDI port");

        #[cfg(not(unix))]
        panic!("Virtual MIDI ports are only supported on macOS/Linux in this example.");

        println!("🎵 Virtual MIDI port 'Expressive MPE Out' created!");
        println!("👉 Please open your DAW (Ableton, Logic) or a Synth and listen to this port.");
        println!("⏳ Starting simulation in 3 seconds...\n");
        sleep(Duration::from_secs(3));

        let mut instrument = MpeInstrument::default();
        let dummy_event = InstrumentEvent::NoteOff {
            channel: Channel::new(0),
            note: U7::new(0),
            velocity: U7::new(0),
        };
        let mut out_buffer = [dummy_event; 16];

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
        println!("✅ PC Simulation complete. Closing port.");
    }
}
