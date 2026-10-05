use embassy_nrf::twim::{self, Twim};
use expressive_core::{PollStatus, SensorError, SensorEvent, SensorReader};

pub struct I2cSensor<'d> {
    i2c: Twim<'d>,
    address: u8,
}

impl<'d> I2cSensor<'d> {
    pub fn new(i2c: Twim<'d>, address: u8) -> Self {
        Self { i2c, address }
    }
}

impl<'d> SensorReader for I2cSensor<'d> {
    fn poll_events(&mut self, buffer: &mut [SensorEvent]) -> Result<PollStatus, SensorError> {
        let mut data = [0u8; 8];

        // Blocking read for simplicity in the polling model.
        // Note: For real async use, SensorReader trait might need to become async
        // or we use non-blocking methods.
        match self.i2c.blocking_read(self.address, &mut data) {
            Ok(_) => {
                // Dummy parsing logic for the hardware sensor:
                // Assuming data[0] is X, data[1] is Y, data[2] is Force.
                if !buffer.is_empty() {
                    let id = 1;
                    buffer[0] = SensorEvent::Touch {
                        id,
                        x: data[0],
                        y: data[1],
                        force: data[2],
                    };
                    Ok(PollStatus {
                        count: 1,
                        more_pending: false,
                    })
                } else {
                    Ok(PollStatus {
                        count: 0,
                        more_pending: true,
                    })
                }
            }
            Err(twim::Error::TxBufferTooLong) | Err(twim::Error::RxBufferTooLong) => {
                Err(SensorError::Overrun)
            }
            Err(_) => Err(SensorError::CommunicationTimeout),
        }
    }
}
