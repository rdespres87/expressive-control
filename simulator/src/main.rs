mod cli;
mod display;
mod error;
mod runner;
mod scenario;

use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use expressive_core::processor::{
    ConfigMessage, ControlProcessor, FilterSpeed, MidiProcessor, ProcessorMode, RootNote, Scale,
};
use expressive_core::{Channel, MidiTransport, TransportError};
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

fn apply_snapshot(ui: &MainWindow, snapshot: &DisplaySnapshot) {
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
    ui.set_is_active(snapshot.is_playing);
    ui.set_frequency(snapshot.pitch_hz.as_str().into());
    ui.set_volume(snapshot.volume_progress);

    // Mock expression differently so it also drops to 0 when muted
    let expr = if snapshot.volume_progress < 0.05 {
        0.0
    } else {
        (1.0 - snapshot.volume_progress).clamp(0.1, 1.0)
    };
    ui.set_expression(expr);

    ui.set_transpose("+0".into()); // Mock for now
}

fn start_and_run<C, A>(
    scenario: &Scenario,
    instrument: &mut C,
    transport: &mut MidirTransport,
    on_action_done: A,
    config_rx: Option<&std::sync::mpsc::Receiver<ConfigMessage>>,
) -> ScenarioReport
where
    C: ControlProcessor,
    A: FnMut(&C),
{
    println!(
        "⏳ Starting simulation in {} seconds...\n",
        START_DELAY.as_secs()
    );
    sleep(START_DELAY);
    run_scenario(
        &scenario.actions,
        instrument,
        transport,
        |_| {},
        on_action_done,
        sleep,
        config_rx,
    )
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
        let mut instrument = MidiProcessor::default();
        let report = start_and_run(&scenario, &mut instrument, &mut transport, |_| {}, None);
        println!("✅ PC Simulation complete: {report:?}");
        return Ok(());
    }

    println!("🎸 Using Theremin Instrument Logic");
    let ui_slave = MainWindow::new().map_err(SimError::Ui)?;
    ui_slave.set_is_master_view(false);
    let ui_master = MainWindow::new().map_err(SimError::Ui)?;
    ui_master.set_is_master_view(true);

    let ui_slave_handle = ui_slave.as_weak();
    // ui_master_handle could be used later

    let (config_tx, config_rx) = std::sync::mpsc::channel();

    let tx_oct = config_tx.clone();
    ui_master.on_octave_changed(move |octave| {
        println!("Master UI [Touch]: Octave changed to {}", octave);
        println!("UART TX (Master -> Slave): SetOctave({})", octave);
        let _ = tx_oct.send(ConfigMessage::SetBaseOctave(octave));
    });

    let tx_scale = config_tx.clone();
    ui_master.on_scale_changed(move |scale| {
        println!("Master UI [Touch]: Scale changed to {}", scale);
        println!("UART TX (Master -> Slave): SetScale({})", scale);
        let s = match scale.as_str() {
            "Chromatic" => Scale::Chromatic,
            "Major" => Scale::Major,
            "Minor" => Scale::Minor,
            "Penta" => Scale::Pentatonic,
            _ => Scale::Chromatic,
        };
        let _ = tx_scale.send(ConfigMessage::SetScale(s));
    });

    let tx_root = config_tx.clone();
    ui_master.on_root_changed(move |root| {
        println!("Master UI [Touch]: Root Note changed to {}", root);
        println!("UART TX (Master -> Slave): SetRootNote({})", root);
        let r = match root.as_str() {
            "C" => RootNote::C,
            "C#" => RootNote::Cs,
            "D" => RootNote::D,
            "D#" => RootNote::Ds,
            "E" => RootNote::E,
            "F" => RootNote::F,
            "F#" => RootNote::Fs,
            "G" => RootNote::G,
            "G#" => RootNote::Gs,
            "A" => RootNote::A,
            "A#" => RootNote::As,
            "B" => RootNote::B,
            _ => RootNote::C,
        };
        let _ = tx_root.send(ConfigMessage::SetRootNote(r));
    });

    let tx_range = config_tx.clone();
    ui_master.on_range_changed(move |r| {
        println!("Master UI [Touch]: Octave Range changed to {}", r);
        println!("UART TX (Master -> Slave): SetOctaveRange({})", r);
        let _ = tx_range.send(ConfigMessage::SetOctaveRange(r));
    });

    let tx_pb = config_tx.clone();
    let ui_master_clone = ui_master.as_weak();
    ui_master.on_pb_range_changed(move |r| {
        if let Some(ui) = ui_master_clone.upgrade() {
            let mode = ui.get_midi_mode();
            println!("Master UI [Touch]: PB Range changed to {}", r);
            let proc_mode = if mode == "MPE" {
                ProcessorMode::Mpe {
                    manager_channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            } else {
                ProcessorMode::Standard {
                    channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            };
            println!(
                "UART TX (Master -> Slave): SetProcessorMode({:?})",
                proc_mode
            );
            let _ = tx_pb.send(ConfigMessage::SetProcessorMode(proc_mode));
        }
    });

    let tx_mode = config_tx.clone();
    let ui_master_clone2 = ui_master.as_weak();
    ui_master.on_midi_mode_changed(move |m| {
        if let Some(ui) = ui_master_clone2.upgrade() {
            let r = ui.get_pb_range();
            println!("Master UI [Touch]: MIDI Mode changed to {}", m);
            let proc_mode = if m == "MPE" {
                ProcessorMode::Mpe {
                    manager_channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            } else {
                ProcessorMode::Standard {
                    channel: Channel::new(0),
                    pitch_bend_range: r as u8,
                }
            };
            println!(
                "UART TX (Master -> Slave): SetProcessorMode({:?})",
                proc_mode
            );
            let _ = tx_mode.send(ConfigMessage::SetProcessorMode(proc_mode));
        }
    });

    let tx_quant = config_tx.clone();
    ui_master.on_quantization_changed(move |q| {
        let _ = tx_quant.send(ConfigMessage::SetPitchQuantization(q));
    });

    let tx_filter = config_tx.clone();
    ui_master.on_filter_changed(move |f| {
        let s = match f.as_str() {
            "Slow" => FilterSpeed::Slow,
            "Med" => FilterSpeed::Med,
            "Fast" => FilterSpeed::Fast,
            _ => FilterSpeed::Fast,
        };
        let _ = tx_filter.send(ConfigMessage::SetFilterSpeed(s));
    });

    // The scenario runs in the background and pushes display snapshots into the Slint event loop.
    std::thread::spawn(move || {
        let mut processor = TofThereminProcessor::default();
        let report = start_and_run(
            &scenario,
            &mut processor,
            &mut transport,
            |proc| {
                let display = ThereminDisplay::from_processor(proc);
                let snapshot = display.snapshot();
                let update =
                    ui_slave_handle.upgrade_in_event_loop(move |ui| apply_snapshot(&ui, &snapshot));
                if let Err(e) = update {
                    eprintln!("  -> UI update failed: {e}");
                }
            },
            Some(&config_rx),
        );
        println!("✅ PC Simulation complete: {report:?}");
    });

    ui_slave.show().map_err(SimError::Ui)?;
    ui_master.show().map_err(SimError::Ui)?;
    slint::run_event_loop().map_err(SimError::Ui)
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
