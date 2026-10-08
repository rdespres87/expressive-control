use crate::{Channel, InstrumentEvent, SensorEvent, U14, U7};

/// Represents a currently active MPE touch.
/// Note: The MPE channel is deliberately excluded to save memory.
/// It is inherently derived from the `ActiveVoice`'s index within the `MpeInstrument` array
/// (where index `i` corresponds to MPE member channel `i + 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveVoice {
    pub touch_id: u16,
    pub base_note: U7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessorError {
    VoiceStealingFailed,
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootNote {
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
    A,
    As,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Chromatic,
    Major,
    Minor,
    Pentatonic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterSpeed {
    Slow,
    Med,
    Fast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessorMode {
    /// Standard MIDI 1.0 mapping. All events are sent on a single channel.
    Standard {
        channel: Channel,
        pitch_bend_range: u8,
    },
    /// MIDI Polyphonic Expression (MPE) mapping.
    Mpe {
        manager_channel: Channel,
        pitch_bend_range: u8,
    },
}

impl Default for ProcessorMode {
    fn default() -> Self {
        ProcessorMode::Standard {
            channel: Channel::new(0),
            pitch_bend_range: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMessage {
    SetRootNote(RootNote),
    SetScale(Scale),
    SetBaseOctave(i32),
    SetOctaveRange(i32),
    SetPitchQuantization(i32),
    SetFilterSpeed(FilterSpeed),
    SetProcessorMode(ProcessorMode),
}

pub trait ControlProcessor {
    /// Handle a configuration change (e.g. from a Master UI or via UART)
    fn handle_config(&mut self, _config: ConfigMessage) {}

    /// Processes a single sensor event.
    /// Writes the resulting MPE events into `out_buffer`.
    /// Returns the number of events written to avoid dynamic allocation.
    fn process_event<T: crate::telemetry::TelemetryHandler>(
        &mut self,
        event: SensorEvent,
        out_buffer: &mut [InstrumentEvent],
        telemetry: &mut T,
    ) -> Result<usize, ProcessorError>;
}

#[derive(Default)]
pub struct MidiProcessor {
    pub voices: [Option<ActiveVoice>; 15],
    pub mode: ProcessorMode,
}

impl ControlProcessor for MidiProcessor {
    fn process_event<T: crate::telemetry::TelemetryHandler>(
        &mut self,
        event: SensorEvent,
        out_buffer: &mut [InstrumentEvent],
        telemetry: &mut T,
    ) -> Result<usize, ProcessorError> {
        telemetry.handle(crate::telemetry::TelemetryEvent::SensorReceived(&event));

        match event {
            SensorEvent::Touch { id, x, y: _, force } => {
                if out_buffer.len() < 2 {
                    return Err(ProcessorError::BufferTooSmall);
                }

                let mut assigned_index = None;
                for i in 0..15 {
                    if self.voices[i].is_none() {
                        assigned_index = Some(i);
                        break;
                    }
                }

                let index = assigned_index.ok_or(ProcessorError::VoiceStealingFailed)?;

                let channel = match self.mode {
                    ProcessorMode::Standard { channel, .. } => channel,
                    ProcessorMode::Mpe {
                        manager_channel, ..
                    } => {
                        // Manager is usually 1, members are 2-16.
                        // Here we just offset by manager_channel value.
                        // A more robust implementation would wrap around or respect standard strictly,
                        // but this satisfies the basic MPE member channel assignment.
                        Channel::new(manager_channel.as_u8() + index as u8 + 1)
                    }
                };

                // We use the initial X coordinate as the base MIDI Note (0-127).
                // The hardware abstraction layer (HAL) will be responsible for scaling physical position to a base note.
                let base_note = U7::new(x);

                self.voices[index] = Some(ActiveVoice {
                    touch_id: id,
                    base_note,
                });

                telemetry.handle(crate::telemetry::TelemetryEvent::VoiceAllocated {
                    touch_id: id,
                    channel: channel.as_u8(),
                });

                out_buffer[0] = InstrumentEvent::NoteOn {
                    channel,
                    note: base_note,
                    velocity: U7::new(force >> 1),
                };

                // In Standard mode, we might just use CC7 (Volume) instead of ChannelPressure,
                // but for now we emit ChannelPressure consistently, or we could branch on mode.
                // For simplicity and expressive control, we keep ChannelPressure for Z.
                out_buffer[1] = InstrumentEvent::ChannelPressure {
                    channel,
                    pressure: U7::new(force >> 1),
                };

                telemetry.handle(crate::telemetry::TelemetryEvent::InstrumentEventGenerated(
                    &out_buffer[0],
                ));
                telemetry.handle(crate::telemetry::TelemetryEvent::InstrumentEventGenerated(
                    &out_buffer[1],
                ));

                Ok(2)
            }
            SensorEvent::Position { id, x, y } => {
                if out_buffer.len() < 2 {
                    return Err(ProcessorError::BufferTooSmall);
                }

                if let Some(index) = self
                    .voices
                    .iter()
                    .position(|v| v.is_some_and(|active| active.touch_id == id))
                {
                    let channel = match self.mode {
                        ProcessorMode::Standard { channel, .. } => channel,
                        ProcessorMode::Mpe {
                            manager_channel, ..
                        } => Channel::new(manager_channel.as_u8() + index as u8 + 1),
                    };

                    // Precise 8-bit to 14-bit scaling:
                    // x << 6 maps the top 8 bits. We fill the lower 6 bits with the top 6 bits of x.
                    let scaled_bend = ((x as u16) << 6) | ((x as u16) >> 2);

                    out_buffer[0] = InstrumentEvent::PitchBend {
                        channel,
                        bend: U14::new(scaled_bend),
                    };

                    out_buffer[1] = InstrumentEvent::Timbre {
                        channel,
                        value: U7::new(y >> 1),
                    };

                    telemetry.handle(crate::telemetry::TelemetryEvent::InstrumentEventGenerated(
                        &out_buffer[0],
                    ));
                    telemetry.handle(crate::telemetry::TelemetryEvent::InstrumentEventGenerated(
                        &out_buffer[1],
                    ));

                    Ok(2)
                } else {
                    Ok(0)
                }
            }
            SensorEvent::Position3D { .. } => Ok(0),
            SensorEvent::Released { id } => {
                if out_buffer.is_empty() {
                    return Err(ProcessorError::BufferTooSmall);
                }

                // Find the voice owned by this touch and free its slot in one step.
                let released = self
                    .voices
                    .iter_mut()
                    .enumerate()
                    .find_map(|(index, slot)| {
                        slot.take_if(|active| active.touch_id == id)
                            .map(|voice| (index, voice))
                    });

                if let Some((index, voice)) = released {
                    let channel = match self.mode {
                        ProcessorMode::Standard { channel, .. } => channel,
                        ProcessorMode::Mpe {
                            manager_channel, ..
                        } => Channel::new(manager_channel.as_u8() + index as u8 + 1),
                    };

                    telemetry.handle(crate::telemetry::TelemetryEvent::VoiceReleased {
                        touch_id: id,
                        channel: channel.as_u8(),
                    });

                    out_buffer[0] = InstrumentEvent::NoteOff {
                        channel,
                        note: voice.base_note,
                        velocity: U7::new(0),
                    };

                    telemetry.handle(crate::telemetry::TelemetryEvent::InstrumentEventGenerated(
                        &out_buffer[0],
                    ));

                    Ok(1)
                } else {
                    Ok(0)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLACEHOLDER: InstrumentEvent = InstrumentEvent::NoteOff {
        channel: Channel::new(0),
        note: U7::new(0),
        velocity: U7::new(0),
    };

    fn touch(id: u16, x: u8, force: u8) -> SensorEvent {
        SensorEvent::Touch { id, x, y: 0, force }
    }

    fn mpe_processor() -> MidiProcessor {
        let mut p = MidiProcessor::default();
        p.mode = ProcessorMode::Mpe {
            manager_channel: Channel::new(0),
            pitch_bend_range: 48,
        };
        p
    }

    #[test]
    fn default_uses_standard_mode() {
        let mpe = MidiProcessor::default();
        assert_eq!(
            mpe.mode,
            ProcessorMode::Standard {
                channel: Channel::new(0),
                pitch_bend_range: 2
            }
        );
        assert!(mpe.voices.iter().all(Option::is_none));
    }

    #[test]
    fn touch_allocates_first_member_channel() {
        let mut mpe = mpe_processor();
        let mut out = [PLACEHOLDER; 4];
        let mut t = crate::telemetry::NoopTelemetry;

        assert_eq!(
            mpe.process_event(touch(42, 64, 200), &mut out, &mut t),
            Ok(2)
        );
        assert_eq!(
            out[0],
            InstrumentEvent::NoteOn {
                channel: Channel::new(1),
                note: U7::new(64),
                velocity: U7::new(100),
            }
        );
        assert_eq!(
            out[1],
            InstrumentEvent::ChannelPressure {
                channel: Channel::new(1),
                pressure: U7::new(100),
            }
        );
        assert_eq!(
            mpe.voices[0],
            Some(ActiveVoice {
                touch_id: 42,
                base_note: U7::new(64),
            })
        );

        assert_eq!(
            mpe.process_event(touch(43, 65, 10), &mut out, &mut t),
            Ok(2)
        );
        assert!(
            matches!(out[0], InstrumentEvent::NoteOn { channel, .. } if channel == Channel::new(2))
        );
    }

    #[test]
    fn sixteenth_touch_cannot_be_allocated() {
        let mut mpe = mpe_processor();
        let mut out = [PLACEHOLDER; 2];
        let mut t = crate::telemetry::NoopTelemetry;
        for id in 0..15 {
            assert_eq!(mpe.process_event(touch(id, 60, 0), &mut out, &mut t), Ok(2));
        }
        assert_eq!(
            mpe.process_event(touch(15, 60, 0), &mut out, &mut t),
            Err(ProcessorError::VoiceStealingFailed)
        );
    }

    #[test]
    fn small_buffers_are_rejected() {
        let mut mpe = mpe_processor();
        let mut one = [PLACEHOLDER; 1];
        let mut none: [InstrumentEvent; 0] = [];
        let mut t = crate::telemetry::NoopTelemetry;

        assert_eq!(
            mpe.process_event(touch(1, 60, 0), &mut one, &mut t),
            Err(ProcessorError::BufferTooSmall)
        );
        assert_eq!(
            mpe.process_event(
                SensorEvent::Position { id: 1, x: 0, y: 0 },
                &mut one,
                &mut t
            ),
            Err(ProcessorError::BufferTooSmall)
        );
        assert_eq!(
            mpe.process_event(SensorEvent::Released { id: 1 }, &mut none, &mut t),
            Err(ProcessorError::BufferTooSmall)
        );
        // Nothing was allocated by the rejected touch.
        assert!(mpe.voices.iter().all(Option::is_none));
    }

    #[test]
    fn position_scales_x_to_14_bit_bend_and_y_to_timbre() {
        let mut mpe = mpe_processor();
        let mut out = [PLACEHOLDER; 2];
        let mut t = crate::telemetry::NoopTelemetry;
        assert_eq!(mpe.process_event(touch(7, 60, 0), &mut out, &mut t), Ok(2));

        for (x, expected) in [(0u8, 0u16), (128, 8224), (255, 16383)] {
            let event = SensorEvent::Position { id: 7, x, y: 200 };
            assert_eq!(mpe.process_event(event, &mut out, &mut t), Ok(2));
            assert_eq!(
                out[0],
                InstrumentEvent::PitchBend {
                    channel: Channel::new(1),
                    bend: U14::new(expected),
                }
            );
            assert_eq!(
                out[1],
                InstrumentEvent::Timbre {
                    channel: Channel::new(1),
                    value: U7::new(100),
                }
            );
        }
    }

    #[test]
    fn unknown_touches_and_3d_positions_are_ignored() {
        let mut mpe = mpe_processor();
        let mut out = [PLACEHOLDER; 2];
        let mut t = crate::telemetry::NoopTelemetry;
        let ignored = [
            SensorEvent::Position { id: 9, x: 1, y: 1 },
            SensorEvent::Released { id: 9 },
            SensorEvent::Position3D {
                id: 0,
                x: 0,
                y: 0,
                z: 100,
            },
        ];
        for event in ignored {
            assert_eq!(mpe.process_event(event, &mut out, &mut t), Ok(0));
        }
    }

    #[test]
    fn release_frees_the_voice_for_reuse() {
        let mut mpe = mpe_processor();
        let mut out = [PLACEHOLDER; 2];
        let mut t = crate::telemetry::NoopTelemetry;
        assert_eq!(mpe.process_event(touch(1, 60, 0), &mut out, &mut t), Ok(2));
        assert_eq!(mpe.process_event(touch(2, 72, 0), &mut out, &mut t), Ok(2));

        assert_eq!(
            mpe.process_event(SensorEvent::Released { id: 1 }, &mut out, &mut t),
            Ok(1)
        );
        assert_eq!(
            out[0],
            InstrumentEvent::NoteOff {
                channel: Channel::new(1),
                note: U7::new(60),
                velocity: U7::new(0),
            }
        );
        assert_eq!(mpe.voices[0], None);
        assert!(mpe.voices[1].is_some());

        // Releasing twice is a no-op.
        assert_eq!(
            mpe.process_event(SensorEvent::Released { id: 1 }, &mut out, &mut t),
            Ok(0)
        );

        // The freed channel is handed to the next touch.
        assert_eq!(mpe.process_event(touch(3, 50, 0), &mut out, &mut t), Ok(2));
        assert!(
            matches!(out[0], InstrumentEvent::NoteOn { channel, .. } if channel == Channel::new(1))
        );
    }
}
