//! RP2040 Coprocessor ("RPE") I2C Protocol Specification.
//!
//! Protocol framing, opcodes, and serialization for:
//! - Real-time USB-MIDI streaming (4-byte packets with CIN 0x08..0x0E)
//! - Front-panel human interface input reports (encoder delta, buttons)
//! - Peripheral GPIO control (ToF XSHUT sequential enable)
//! - Device lifecycle & OTA status queries

/// I2C Target Address for the RP2040 Coprocessor.
pub const RPE_I2C_ADDR: u8 = 0x40;

/// Protocol command opcodes sent by the Controller (ESP32).
pub mod cmd {
    /// Query bootloader / application status and firmware metadata.
    pub const GET_STATUS: u8 = 0x01;
    /// Command bootloader to jump to application slot.
    pub const BOOT_APP: u8 = 0x05;
    /// Request front-panel human input report (encoder delta and button states).
    pub const READ_INPUTS: u8 = 0x10;
    /// Set ToF sensor XSHUT enable pins (payload: 1 byte bitmask).
    pub const SET_XSHUT: u8 = 0x20;
    /// Request soft reboot into bootloader mode for OTA flashing.
    pub const ENTER_BOOTLOADER: u8 = 0xFB;
}

/// Bitmask definitions for ToF XSHUT GPIO control (`cmd::SET_XSHUT`).
pub mod xshut {
    pub const NONE: u8 = 0x00;
    /// Left ToF sensor (Volume arm) on GP2.
    pub const LEFT_TOF: u8 = 0x01;
    /// Right ToF sensor (Pitch arm) on GP3.
    pub const RIGHT_TOF: u8 = 0x02;
    /// Both ToF sensors active.
    pub const BOTH: u8 = LEFT_TOF | RIGHT_TOF;
}

/// Bitmask definitions for front-panel buttons (`RpeInputReport::button_state`).
pub mod buttons {
    /// Rotary encoder push switch (GP8).
    pub const ENCODER_SW: u8 = 0x01;
    /// Octave + button (GP14).
    pub const OCTAVE_UP: u8 = 0x02;
    /// Octave - button (GP15).
    pub const OCTAVE_DOWN: u8 = 0x04;
    /// Audio mute button (GP26).
    pub const MUTE: u8 = 0x08;
}

/// Status flags returned in `RpeInputReport::status_flags`.
pub mod flags {
    /// Host USB-MIDI interface is configured and active.
    pub const USB_CONFIGURED: u8 = 0x01;
    /// Left ToF XSHUT pin is driven HIGH.
    pub const XSHUT_LEFT_ACTIVE: u8 = 0x02;
    /// Right ToF XSHUT pin is driven HIGH.
    pub const XSHUT_RIGHT_ACTIVE: u8 = 0x04;
}

/// 4-byte atomic input report returned by RP2040 upon `cmd::READ_INPUTS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct RpeInputReport {
    /// Accumulated encoder pulse delta since last read (+1 = CW, -1 = CCW).
    pub encoder_delta: i8,
    /// Instantaneous debounced state of all buttons (1 = pressed, 0 = released).
    pub button_state: u8,
    /// Latched button click events since last read (1 = clicked).
    pub button_clicks: u8,
    /// Auxiliary system status flags (USB connection, XSHUT states).
    pub status_flags: u8,
}

impl RpeInputReport {
    /// Serialize report into a 4-byte buffer for I2C transmission.
    pub const fn to_bytes(&self) -> [u8; 4] {
        [
            self.encoder_delta as u8,
            self.button_state,
            self.button_clicks,
            self.status_flags,
        ]
    }

    /// Deserialize report from a 4-byte I2C response buffer.
    pub const fn from_bytes(bytes: &[u8; 4]) -> Self {
        Self {
            encoder_delta: bytes[0] as i8,
            button_state: bytes[1],
            button_clicks: bytes[2],
            status_flags: bytes[3],
        }
    }

    /// Check if a specific button is currently held down.
    pub const fn is_button_pressed(&self, mask: u8) -> bool {
        (self.button_state & mask) != 0
    }

    /// Check if a specific button registered a click transition since last read.
    pub const fn has_button_clicked(&self, mask: u8) -> bool {
        (self.button_clicks & mask) != 0
    }

    /// Check if the rotary encoder pushbutton is pressed.
    pub const fn is_encoder_sw_pressed(&self) -> bool {
        self.is_button_pressed(buttons::ENCODER_SW)
    }

    /// Check if the Octave + button is pressed.
    pub const fn is_octave_up_pressed(&self) -> bool {
        self.is_button_pressed(buttons::OCTAVE_UP)
    }

    /// Check if the Octave - button is pressed.
    pub const fn is_octave_down_pressed(&self) -> bool {
        self.is_button_pressed(buttons::OCTAVE_DOWN)
    }

    /// Check if the Mute button is pressed.
    pub const fn is_mute_pressed(&self) -> bool {
        self.is_button_pressed(buttons::MUTE)
    }

    /// Check if the USB-MIDI host connection is active.
    pub const fn is_usb_configured(&self) -> bool {
        (self.status_flags & flags::USB_CONFIGURED) != 0
    }
}

/// Classification of incoming I2C write payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpeIncomingPacket<'a> {
    StatusQuery,
    BootApp,
    ReadInputsRequest,
    EnterBootloader,
    SetXshut { mask: u8 },
    UsbMidi(&'a [u8]),
    Unknown,
}

/// Classify an incoming I2C write payload based on length and opcode.
pub fn classify_incoming_packet(rx: &[u8]) -> RpeIncomingPacket<'_> {
    if rx.is_empty() {
        return RpeIncomingPacket::Unknown;
    }
    match rx {
        [cmd::GET_STATUS] => RpeIncomingPacket::StatusQuery,
        [cmd::BOOT_APP] => RpeIncomingPacket::BootApp,
        [cmd::READ_INPUTS] => RpeIncomingPacket::ReadInputsRequest,
        [cmd::ENTER_BOOTLOADER] => RpeIncomingPacket::EnterBootloader,
        [cmd::SET_XSHUT, mask] => RpeIncomingPacket::SetXshut { mask: *mask },
        _ => {
            // Check for valid USB-MIDI packet stream:
            // Length must be a positive multiple of 4.
            // First byte CIN must be a standard Channel Voice message (0x08..=0x0E).
            // Second byte status must have MSB set (0x80..=0xEF).
            if rx.len() >= 4 && rx.len().is_multiple_of(4) {
                let first_cin = rx[0] & 0x0F;
                let first_status = rx[1];
                if (0x08..=0x0E).contains(&first_cin) && (first_status & 0x80) != 0 {
                    return RpeIncomingPacket::UsbMidi(rx);
                }
            }
            RpeIncomingPacket::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_serialization_round_trip() {
        let original = RpeInputReport {
            encoder_delta: -3,
            button_state: buttons::OCTAVE_UP | buttons::MUTE,
            button_clicks: buttons::OCTAVE_UP,
            status_flags: flags::USB_CONFIGURED | flags::XSHUT_LEFT_ACTIVE,
        };

        let bytes = original.to_bytes();
        assert_eq!(bytes[0], (-3i8) as u8);
        assert_eq!(bytes[1], 0x02 | 0x08);
        assert_eq!(bytes[2], 0x02);
        assert_eq!(bytes[3], 0x01 | 0x02);

        let decoded = RpeInputReport::from_bytes(&bytes);
        assert_eq!(decoded, original);
        assert_eq!(decoded.encoder_delta, -3);
        assert!(decoded.is_octave_up_pressed());
        assert!(decoded.is_mute_pressed());
        assert!(!decoded.is_octave_down_pressed());
        assert!(!decoded.is_encoder_sw_pressed());
        assert!(decoded.has_button_clicked(buttons::OCTAVE_UP));
        assert!(!decoded.has_button_clicked(buttons::OCTAVE_DOWN));
        assert!(decoded.is_usb_configured());
    }

    #[test]
    fn test_classify_control_commands() {
        assert_eq!(
            classify_incoming_packet(&[cmd::GET_STATUS]),
            RpeIncomingPacket::StatusQuery
        );
        assert_eq!(
            classify_incoming_packet(&[cmd::BOOT_APP]),
            RpeIncomingPacket::BootApp
        );
        assert_eq!(
            classify_incoming_packet(&[cmd::READ_INPUTS]),
            RpeIncomingPacket::ReadInputsRequest
        );
        assert_eq!(
            classify_incoming_packet(&[cmd::ENTER_BOOTLOADER]),
            RpeIncomingPacket::EnterBootloader
        );
        assert_eq!(
            classify_incoming_packet(&[cmd::SET_XSHUT, xshut::BOTH]),
            RpeIncomingPacket::SetXshut { mask: 0x03 }
        );
        assert_eq!(
            classify_incoming_packet(&[cmd::SET_XSHUT, xshut::LEFT_TOF]),
            RpeIncomingPacket::SetXshut { mask: 0x01 }
        );
    }

    #[test]
    fn test_classify_usb_midi_packets() {
        // Single NoteOn: CIN 9, Status 0x90, Note 60, Vel 100
        let note_on = [0x09, 0x90, 60, 100];
        assert_eq!(
            classify_incoming_packet(&note_on),
            RpeIncomingPacket::UsbMidi(&note_on)
        );

        // Pitch Bend: CIN 0x0E, Status 0xE0, LSB 0, MSB 64
        let pitch_bend = [0x0E, 0xE0, 0x00, 0x40];
        assert_eq!(
            classify_incoming_packet(&pitch_bend),
            RpeIncomingPacket::UsbMidi(&pitch_bend)
        );

        // CC7 Volume: CIN 0x0B, Status 0xB0, CC 7, Val 120
        let cc7 = [0x0B, 0xB0, 0x07, 120];
        assert_eq!(
            classify_incoming_packet(&cc7),
            RpeIncomingPacket::UsbMidi(&cc7)
        );

        // Batch of 2 packets (8 bytes)
        let batch = [0x0B, 0xB0, 0x07, 120, 0x0B, 0xB0, 0x0B, 100];
        assert_eq!(
            classify_incoming_packet(&batch),
            RpeIncomingPacket::UsbMidi(&batch)
        );
    }

    #[test]
    fn test_classify_invalid_or_unknown_packets() {
        assert_eq!(classify_incoming_packet(&[]), RpeIncomingPacket::Unknown);
        // Odd length
        assert_eq!(
            classify_incoming_packet(&[0x09, 0x90, 60]),
            RpeIncomingPacket::Unknown
        );
        // Invalid CIN for MIDI
        assert_eq!(
            classify_incoming_packet(&[0x01, 0x90, 60, 100]),
            RpeIncomingPacket::Unknown
        );
        // Status byte MSB not set
        assert_eq!(
            classify_incoming_packet(&[0x09, 0x10, 60, 100]),
            RpeIncomingPacket::Unknown
        );
        // Random unknown command
        assert_eq!(
            classify_incoming_packet(&[0xAA, 0xBB]),
            RpeIncomingPacket::Unknown
        );
    }
}
