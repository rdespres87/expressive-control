#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// MasterMessage: Sent from the Master board (ToF sensors) to the Slave board (Screen)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MasterMessage {
    /// Telemetry data sent at ~50Hz
    Telemetry {
        midi_note: u8,
        pitch_bend_cents: i8,
        volume: u8,
        is_muted: bool,
    },
    /// Acknowledgment of a config change
    ConfigAck,
}

/// SlaveMessage: Sent from the Slave board (UI) to the Master board (Logic)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SlaveMessage {
    SetScale(ScaleType),
    SetBaseOctave(i8),
    SetSensitivity { sensor: SensorType, value: u8 },
    Heartbeat,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ScaleType {
    Chromatic,
    Major,
    Minor,
    Pentatonic,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SensorType {
    Pitch,
    Volume,
}

/// Maximum size of a serialized and COBS-encoded message in bytes.
/// Postcard is very compact. 64 bytes is more than enough for these small enums.
pub const MAX_MESSAGE_SIZE: usize = 64;
