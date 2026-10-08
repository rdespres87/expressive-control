use crate::{InstrumentEvent, SensorEvent};

/// Telemetry events that can be logged without allocating strings in the core.
#[derive(Debug)]
pub enum TelemetryEvent<'a> {
    SensorReceived(&'a SensorEvent),
    InstrumentEventGenerated(&'a InstrumentEvent),
    VoiceAllocated { touch_id: u16, channel: u8 },
    VoiceReleased { touch_id: u16, channel: u8 },
    Error(&'a str),
}

/// A trait for handling telemetry events.
/// Implemented by the target platform (e.g. using `log` for ESP32, or `defmt` for nRF5340).
pub trait TelemetryHandler {
    fn handle(&mut self, event: TelemetryEvent);
}

/// A no-op implementation for testing or when telemetry is disabled.
pub struct NoopTelemetry;

impl TelemetryHandler for NoopTelemetry {
    fn handle(&mut self, _event: TelemetryEvent) {}
}
