#![no_std]
#![no_main]

use defmt_rtt as _; // global logger
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use expressive_core::{Channel, InstrumentEvent, SensorEvent, U7};
use expressive_core::midi::ToMidi;
use expressive_core::processor::{ControlProcessor, MpeInstrument};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_nrf::init(Default::default());

    defmt::info!("Expressive Control nRF5340 firmware starting!");

    let mut instrument = MpeInstrument::default();
    
    // Dummy buffer initialization
    let dummy_event = InstrumentEvent::NoteOff { channel: Channel::new(0), note: U7::new(0), velocity: U7::new(0) };
    let mut out_buffer = [dummy_event; 16];

    loop {
        // Poll sensor mock
        let mock_event = SensorEvent::Touch { id: 1, x: 64, y: 0, force: 100 };

        if let Ok(count) = instrument.process_event(mock_event, &mut out_buffer) {
            for event_out in out_buffer.iter().take(count) {
                let midi_msg = event_out.to_midi();
                defmt::info!("MIDI Out: {:?}", midi_msg.as_slice());
            }
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
