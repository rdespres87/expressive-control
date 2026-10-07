mod cli;
mod display;
mod error;
mod runner;
mod scenario;

use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use expressive_core::processor::{ControlProcessor, MpeInstrument};
use expressive_core::{InstrumentEvent, MidiTransport, TransportError};
#[cfg(unix)]
use midir::os::unix::VirtualOutput;
use midir::MidiOutputConnection;
use theremin::TofThereminProcessor;

use crate::cli::CliArgs;
use crate::display::{DisplaySnapshot, ThereminDisplay};
use crate::error::SimError;
use crate::runner::{run_hil, run_scenario, ScenarioReport};
use crate::scenario::Scenario;

slint::include_modules!();

/// Delay before replaying a scenario, leaving time to arm a DAW or a logic analyser.
const START_DELAY: Duration = Duration::from_secs(3);
const SERIAL_BAUD_RATE: u32 = 115_200;
const SERIAL_TIMEOUT: Duration = Duration::from_millis(100);
const MIDI_CLIENT_NAME: &str = "Expressive Simulator";
const MIDI_PORT_NAME: &str = "Expressive MPE Out";

/// `MidiTransport` backed by a midir output connection.
struct MidirTransport(MidiOutputConnection);

impl MidiTransport for MidirTransport {
    fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.0.send(bytes).map_err(|e| {
            eprintln!("  -> midir send error: {e}");
            TransportError::HardwareDisconnected
        })
    }
}

#[cfg(unix)]
fn open_virtual_midi_port() -> Result<MidiOutputConnection, SimError> {
    let midi_out = midir::MidiOutput::new(MIDI_CLIENT_NAME).map_err(SimError::MidiInit)?;
    midi_out
        .create_virtual(MIDI_PORT_NAME)
        .map_err(|e| SimError::MidiConnect(e.to_string()))
}

#[cfg(not(unix))]
fn open_virtual_midi_port() -> Result<MidiOutputConnection, SimError> {
    Err(SimError::UnsupportedPlatform)
}

fn apply_snapshot(ui: &SlaveWindow, snapshot: &DisplaySnapshot) {
    // Note: pitch_note is e.g. "C#4" or "D5"
    // We split it into name ("C#") and octave ("4")
    let note_str = snapshot.pitch_note.as_str();
    let mut name = String::new();
    let mut octave = String::new();
    for c in note_str.chars() {
        if c.is_ascii_digit() || (c == '-' && octave.is_empty()) {
            octave.push(c);
        } else {
            name.push(c);
        }
    }
    ui.set_note_name(name.into());
    ui.set_octave(octave.into());
    ui.set_cents(snapshot.pitch_cents);
    ui.set_is_active(snapshot.volume_progress > 0.0);
}

fn start_and_run<F>(
    scenario: &Scenario,
    instrument: &mut dyn ControlProcessor,
    transport: &mut MidirTransport,
    on_event: F,
) -> ScenarioReport
where
    F: FnMut(&InstrumentEvent),
{
    println!(
        "⏳ Starting simulation in {} seconds...\n",
        START_DELAY.as_secs()
    );
    sleep(START_DELAY);
    run_scenario(&scenario.actions, instrument, transport, on_event, sleep)
}

fn run_hil_mode(port_name: &str, scenario: &Scenario) -> Result<(), SimError> {
    println!("🎵 HIL Mode: Opening serial port {port_name}...");
    let mut port = serialport::new(port_name, SERIAL_BAUD_RATE)
        .timeout(SERIAL_TIMEOUT)
        .open()
        .map_err(SimError::SerialOpen)?;

    println!(
        "⏳ Starting HIL simulation in {} seconds...\n",
        START_DELAY.as_secs()
    );
    sleep(START_DELAY);
    let report = run_hil(&scenario.actions, &mut port, sleep);
    println!("✅ HIL Simulation complete: {report:?}");
    Ok(())
}

fn run_pc_mode(scenario: Scenario, is_theremin: bool) -> Result<(), SimError> {
    println!("💻 PC Simulation Mode");
    let mut transport = MidirTransport(open_virtual_midi_port()?);
    println!("🎵 Virtual MIDI port '{MIDI_PORT_NAME}' created!");
    println!("👉 Please open your DAW (Ableton, Logic) or a Synth and listen to this port.");

    if !is_theremin {
        println!("🎹 Using MPE Keyboard Logic");
        let mut instrument = MpeInstrument::default();
        let report = start_and_run(&scenario, &mut instrument, &mut transport, |_| {});
        println!("✅ PC Simulation complete: {report:?}");
        return Ok(());
    }

    println!("🎸 Using Theremin Instrument Logic");
    let ui = SlaveWindow::new().map_err(SimError::Ui)?;
    let ui_handle = ui.as_weak();

    // The UI owns the main thread; the scenario runs in the background and pushes
    // display snapshots into the Slint event loop.
    std::thread::spawn(move || {
        let mut processor = TofThereminProcessor::default();
        let mut display = ThereminDisplay::from_processor(&processor);
        let report = start_and_run(&scenario, &mut processor, &mut transport, |event| {
            if display.apply(event) {
                let snapshot = display.snapshot();
                let update =
                    ui_handle.upgrade_in_event_loop(move |ui| apply_snapshot(&ui, &snapshot));
                if let Err(e) = update {
                    eprintln!("  -> UI update failed: {e}");
                }
            }
        });
        println!("✅ PC Simulation complete: {report:?}");
    });

    ui.run().map_err(SimError::Ui)
}

fn run(args: CliArgs) -> Result<(), SimError> {
    println!("📄 Loading scenario: {}", args.scenario_path);
    let scenario = Scenario::load(&args.scenario_path)?;
    match args.serial_port.as_deref() {
        Some(port_name) => run_hil_mode(port_name, &scenario),
        None => run_pc_mode(scenario, args.is_theremin()),
    }
}

fn main() -> ExitCode {
    match run(CliArgs::parse(std::env::args())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("❌ {e}");
            ExitCode::FAILURE
        }
    }
}
