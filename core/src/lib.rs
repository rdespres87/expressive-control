#![no_std]

pub mod midi;

pub mod processor;

// --- Type Safety Wrappers ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct U7(u8);
impl U7 {
    pub const fn new(val: u8) -> Self {
        Self(if val > 127 { 127 } else { val })
    }
    pub const fn as_u8(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct U14(u16);
impl U14 {
    pub const fn new(val: u16) -> Self {
        Self(if val > 16383 { 16383 } else { val })
    }
    pub const fn as_u16(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel(u8);
impl Channel {
    pub const fn new(val: u8) -> Self {
        Self(if val > 15 { 15 } else { val })
    }
    pub const fn as_u8(&self) -> u8 {
        self.0
    }
}

// --- Events ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SensorEvent {
    Touch { id: u16, force: u8 },
    Position { id: u16, x: u8, y: u8 },
    Released { id: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentEvent {
    NoteOn {
        channel: Channel,
        note: U7,
        velocity: U7,
    },
    NoteOff {
        channel: Channel,
        note: U7,
        velocity: U7,
    },
    ChannelPressure {
        channel: Channel,
        pressure: U7,
    },
    PitchBend {
        channel: Channel,
        bend: U14,
    },
    Timbre {
        channel: Channel,
        value: U7,
    },
}

// --- Errors ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorError {
    HardwareDisconnected,
    CommunicationTimeout,
    Overrun, // Hardware FIFO overflowed before we could poll
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    TxBufferFull,
    HardwareDisconnected,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollStatus {
    pub count: usize,
    pub more_pending: bool, // True if the buffer was filled but HW still has events
}

// --- Interfaces ---

pub trait SensorReader {
    /// Polls the hardware for new events.
    fn poll_events(&mut self, buffer: &mut [SensorEvent]) -> Result<PollStatus, SensorError>;
}

pub trait MidiTransport {
    /// Sends raw MIDI bytes out to the physical or virtual port.
    fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
}

pub fn test_core() -> bool {
    true
}
