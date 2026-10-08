use std::io::Write;
use std::time::Duration;

use expressive_core::midi::ToMidi;
use expressive_core::processor::ControlProcessor;
use expressive_core::{Channel, InstrumentEvent, MidiTransport, SensorEvent, U7};

use crate::error::SimError;
use crate::scenario::SimulatorAction;

/// Maximum number of instrument events produced by a single sensor event.
pub const OUT_BUFFER_LEN: usize = 16;
/// Size of the COBS frame buffer used to inject events over the HIL serial link.
pub const COBS_BUFFER_LEN: usize = 32;

/// Counters describing how a scenario run went. Errors never abort a run: they are
/// logged, counted, and the next action is processed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioReport {
    pub events_in: usize,
    pub events_out: usize,
    pub processing_errors: usize,
    pub transport_errors: usize,
}

/// Replays `actions` through `instrument` and sends the resulting MIDI bytes to
/// `transport`. `on_event` is called for every produced instrument event (UI hook)
/// and `wait` performs the scenario pauses (injectable so tests do not sleep).
pub fn run_scenario<C, T, F, A, W>(
    actions: &[SimulatorAction],
    instrument: &mut C,
    transport: &mut T,
    mut on_event: F,
    mut on_action_done: A,
    mut wait: W,
) -> ScenarioReport
where
    C: ControlProcessor,
    T: MidiTransport + ?Sized,
    F: FnMut(&InstrumentEvent),
    A: FnMut(&C),
    W: FnMut(Duration),
{
    let placeholder = InstrumentEvent::NoteOff {
        channel: Channel::new(0),
        note: U7::new(0),
        velocity: U7::new(0),
    };
    let mut out_buffer = [placeholder; OUT_BUFFER_LEN];
    let mut report = ScenarioReport::default();
    let mut t = expressive_core::telemetry::NoopTelemetry;

    for action in actions {
        if let Some(event) = action.event {
            report.events_in += 1;
            println!("Input: {event:?}");
            match instrument.process_event(event, &mut out_buffer, &mut t) {
                Ok(count) => {
                    for event_out in out_buffer.iter().take(count) {
                        report.events_out += 1;
                        let midi_msg = event_out.to_midi();
                        println!("  -> {event_out:?} | Hex: {:02X?}", midi_msg.as_slice());
                        if let Err(e) = transport.send_bytes(midi_msg.as_slice()) {
                            report.transport_errors += 1;
                            eprintln!("  -> Error sending MIDI: {e:?}");
                        }
                        on_event(event_out);
                    }
                }
                Err(e) => {
                    report.processing_errors += 1;
                    eprintln!("  -> Processing error: {e:?}");
                }
            }
        }

        on_action_done(instrument);

        if let Some(ms) = action.wait_ms {
            wait(Duration::from_millis(ms));
        }
    }
    report
}

/// Encodes a sensor event as a postcard + COBS frame (format expected by the HIL firmware).
pub fn encode_event<'b>(event: &SensorEvent, buf: &'b mut [u8]) -> Result<&'b mut [u8], SimError> {
    postcard::to_slice_cobs(event, buf).map_err(SimError::Encode)
}

/// Injects every scenario event into the firmware through `port` (HIL mode).
pub fn run_hil<P, W>(actions: &[SimulatorAction], port: &mut P, mut wait: W) -> ScenarioReport
where
    P: Write + ?Sized,
    W: FnMut(Duration),
{
    let mut report = ScenarioReport::default();
    for action in actions {
        if let Some(event) = action.event {
            report.events_in += 1;
            println!("Injecting: {event:?}");
            let mut buf = [0u8; COBS_BUFFER_LEN];
            match encode_event(&event, &mut buf) {
                Ok(frame) => {
                    if let Err(e) = port.write_all(frame) {
                        report.transport_errors += 1;
                        eprintln!("  -> Error sending to Serial: {e}");
                    } else {
                        report.events_out += 1;
                    }
                }
                Err(e) => {
                    report.processing_errors += 1;
                    eprintln!("  -> {e}");
                }
            }
        }

        if let Some(ms) = action.wait_ms {
            wait(Duration::from_millis(ms));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use expressive_core::processor::MpeInstrument;
    use expressive_core::TransportError;
    use theremin::TofThereminProcessor;

    #[derive(Default)]
    struct RecordingTransport {
        sent: Vec<Vec<u8>>,
    }

    impl MidiTransport for RecordingTransport {
        fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            self.sent.push(bytes.to_vec());
            Ok(())
        }
    }

    struct FailingTransport;

    impl MidiTransport for FailingTransport {
        fn send_bytes(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
            Err(TransportError::HardwareDisconnected)
        }
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "unplugged",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn event(event: SensorEvent) -> SimulatorAction {
        SimulatorAction {
            event: Some(event),
            wait_ms: None,
        }
    }

    fn pause(ms: u64) -> SimulatorAction {
        SimulatorAction {
            event: None,
            wait_ms: Some(ms),
        }
    }

    fn pitch(z: u16) -> SensorEvent {
        SensorEvent::Position3D {
            id: 0,
            x: 0,
            y: 0,
            z,
        }
    }

    #[test]
    fn theremin_scenario_produces_midi_and_ui_events() {
        let actions = [
            event(SensorEvent::Position3D {
                id: 1,
                x: 0,
                y: 0,
                z: 20,
            }), // Set volume
            pause(10),
            event(pitch(1000)),
            pause(20),
            event(pitch(20)),
            pause(30),
        ];
        let mut processor = TofThereminProcessor::default();
        let mut transport = RecordingTransport::default();
        let mut ui_events = Vec::new();
        let mut waits = Vec::new();

        let report = run_scenario(
            &actions,
            &mut processor,
            &mut transport,
            |e| ui_events.push(*e),
            |_| {}, // ignore action done
            |d| waits.push(d),
        );

        // Volume event generates nothing (!is_playing).
        // First pitch generates NoteOn + PitchBend. Second pitch generates NoteOff + NoteOn + PitchBend.
        assert_eq!(
            report,
            ScenarioReport {
                events_in: 3,
                events_out: 5,
                processing_errors: 0,
                transport_errors: 0,
            }
        );
        assert_eq!(
            transport.sent,
            vec![
                vec![0x91, 60, 127], // NoteOn 60, vel 127
                vec![0xE1, 0x00, 0x40],
                vec![0x81, 60, 0],
                vec![0x91, 84, 127],
                vec![0xE1, 0x00, 0x40]
            ]
        );
        assert_eq!(ui_events.len(), 5);
        assert_eq!(
            waits,
            vec![
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(30)
            ]
        );
    }

    #[test]
    fn transport_errors_are_counted_not_fatal() {
        let actions = [
            event(SensorEvent::Position3D {
                id: 1,
                x: 0,
                y: 0,
                z: 20,
            }), // Set volume
            event(pitch(500)),
            event(pitch(400)),
        ];
        let mut processor = TofThereminProcessor::default();
        let mut ui_calls = 0;

        let report = run_scenario(
            &actions,
            &mut processor,
            &mut FailingTransport,
            |_| ui_calls += 1,
            |_| {},
            |_| {},
        );

        // Volume event generates nothing (!is_playing).
        // First pitch: NoteOn + PitchBend (2). Second pitch: NoteOff + NoteOn + PitchBend (3).
        assert_eq!(report.events_out, 5);
        assert_eq!(report.transport_errors, 5);
        // The UI still mirrors what the instrument produced.
        assert_eq!(ui_calls, 5);
    }

    #[test]
    fn processing_errors_are_counted_not_fatal() {
        // 15 MPE voices are available: the 16th touch cannot be allocated.
        let actions: Vec<_> = (0..16)
            .map(|id| {
                event(SensorEvent::Touch {
                    id,
                    x: 60,
                    y: 0,
                    force: 100,
                })
            })
            .collect();
        let mut instrument = MpeInstrument::default();
        let mut transport = RecordingTransport::default();

        let report = run_scenario(
            &actions,
            &mut instrument,
            &mut transport,
            |_| {},
            |_| {},
            |_| {},
        );

        assert_eq!(report.events_in, 16);
        assert_eq!(report.processing_errors, 1);
        assert_eq!(report.events_out, 30);
        assert_eq!(transport.sent.len(), 30);
    }

    #[test]
    fn empty_scenario_does_nothing() {
        let mut processor = TofThereminProcessor::default();
        let mut transport = RecordingTransport::default();
        let report = run_scenario(&[], &mut processor, &mut transport, |_| {}, |_| {}, |_| {});
        assert_eq!(report, ScenarioReport::default());
        assert!(transport.sent.is_empty());
    }

    #[test]
    fn encoded_event_round_trips() {
        let original = SensorEvent::Touch {
            id: 7,
            x: 1,
            y: 2,
            force: 3,
        };
        let mut buf = [0u8; COBS_BUFFER_LEN];
        let frame = encode_event(&original, &mut buf).unwrap();
        assert_eq!(frame.last(), Some(&0), "COBS frames end with a 0 delimiter");
        let decoded: SensorEvent = postcard::from_bytes_cobs(frame).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn encoding_into_a_too_small_buffer_fails() {
        let mut buf = [0u8; 2];
        let err = encode_event(&pitch(1234), &mut buf).unwrap_err();
        assert!(matches!(err, SimError::Encode(_)), "{err:?}");
    }

    #[test]
    fn hil_writes_one_frame_per_event() {
        let actions = [
            event(pitch(100)),
            pause(5),
            event(SensorEvent::Released { id: 0 }),
        ];
        let mut port = Vec::new();
        let mut waits = Vec::new();

        let report = run_hil(&actions, &mut port, |d| waits.push(d));

        assert_eq!(report.events_in, 2);
        assert_eq!(report.events_out, 2);
        assert_eq!(report.transport_errors, 0);
        assert_eq!(waits, vec![Duration::from_millis(5)]);

        let mut frames: Vec<Vec<u8>> = port
            .split_inclusive(|b| *b == 0)
            .map(<[u8]>::to_vec)
            .collect();
        assert_eq!(frames.len(), 2);
        let first: SensorEvent = postcard::from_bytes_cobs(&mut frames[0]).unwrap();
        let second: SensorEvent = postcard::from_bytes_cobs(&mut frames[1]).unwrap();
        assert_eq!(first, pitch(100));
        assert_eq!(second, SensorEvent::Released { id: 0 });
    }

    #[test]
    fn hil_write_errors_are_counted_not_fatal() {
        let actions = [event(pitch(100)), event(pitch(200))];
        let report = run_hil(&actions, &mut FailingWriter, |_| {});
        assert_eq!(report.events_in, 2);
        assert_eq!(report.events_out, 0);
        assert_eq!(report.transport_errors, 2);
    }
}
