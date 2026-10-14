//! Microchip MCP23017 16-bit I2C I/O Expander Driver
//!
//! A completely generic, hardware-isolated driver for the Microchip MCP23017
//! 16-bit I/O expander. Provides register configuration, port-level, and
//! pin-level operations across Port A and Port B.

use esp_idf_hal::i2c::I2cDriver;
use esp_idf_svc::sys::EspError;

/// Default I2C address when A0=0, A1=0, A2=0.
pub const MCP23017_DEFAULT_ADDR: u8 = 0x20;

/// Complete MCP23017 Register Map (when IOCON.BANK = 0).
pub mod regs {
    pub const IODIRA: u8 = 0x00;
    pub const IODIRB: u8 = 0x01;
    pub const IPOLA: u8 = 0x02;
    pub const IPOLB: u8 = 0x03;
    pub const GPINTENA: u8 = 0x04;
    pub const GPINTENB: u8 = 0x05;
    pub const DEFVALA: u8 = 0x06;
    pub const DEFVALB: u8 = 0x07;
    pub const INTCONA: u8 = 0x08;
    pub const INTCONB: u8 = 0x09;
    pub const IOCON: u8 = 0x0A;
    pub const GPPUA: u8 = 0x0C;
    pub const GPPUB: u8 = 0x0D;
    pub const INTFA: u8 = 0x0E;
    pub const INTFB: u8 = 0x0F;
    pub const INTCAPA: u8 = 0x10;
    pub const INTCAPB: u8 = 0x11;
    pub const GPIOA: u8 = 0x12;
    pub const GPIOB: u8 = 0x13;
    pub const OLATA: u8 = 0x14;
    pub const OLATB: u8 = 0x15;

    // IOCON Configuration Bits
    pub const IOCON_BANK: u8 = 1 << 7; // 1 = Separate register banks, 0 = Sequential
    pub const IOCON_MIRROR: u8 = 1 << 6; // 1 = INTA/INTB mirrored, 0 = Functional
    pub const IOCON_SEQOP: u8 = 1 << 5; // 1 = Address pointer disabled, 0 = Auto-increment
    pub const IOCON_DISSLW: u8 = 1 << 4; // 1 = Slew rate disabled, 0 = Enabled
    pub const IOCON_HAEN: u8 = 1 << 3; // Hardware Address Enable (SPI only)
    pub const IOCON_ODR: u8 = 1 << 2; // 1 = Open-drain output, 0 = Active driver
    pub const IOCON_INTPOL: u8 = 1 << 1; // 1 = Active-high interrupt, 0 = Active-low
}

/// Port selection on MCP23017.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    A,
    B,
}

/// Digital I/O Direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// Individual Pin on MCP23017.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pin {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    B0,
    B1,
    B2,
    B3,
    B4,
    B5,
    B6,
    B7,
}

impl Pin {
    /// Returns the port (A or B) and the bit index (0 to 7) for this pin.
    pub const fn port_and_index(self) -> (Port, u8) {
        match self {
            Self::A0 => (Port::A, 0),
            Self::A1 => (Port::A, 1),
            Self::A2 => (Port::A, 2),
            Self::A3 => (Port::A, 3),
            Self::A4 => (Port::A, 4),
            Self::A5 => (Port::A, 5),
            Self::A6 => (Port::A, 6),
            Self::A7 => (Port::A, 7),
            Self::B0 => (Port::B, 0),
            Self::B1 => (Port::B, 1),
            Self::B2 => (Port::B, 2),
            Self::B3 => (Port::B, 3),
            Self::B4 => (Port::B, 4),
            Self::B5 => (Port::B, 5),
            Self::B6 => (Port::B, 6),
            Self::B7 => (Port::B, 7),
        }
    }

    /// Returns the bitmask (1 << index) for this pin.
    pub const fn mask(self) -> u8 {
        let (_, idx) = self.port_and_index();
        1 << idx
    }
}

/// Generic, decoupled driver for the Microchip MCP23017 16-bit I/O Expander.
pub struct Mcp23017Driver {
    addr: u8,
    olat_a: u8,
    olat_b: u8,
    iodir_a: u8,
    iodir_b: u8,
    gppu_a: u8,
    gppu_b: u8,
}

impl Default for Mcp23017Driver {
    fn default() -> Self {
        Self::new(MCP23017_DEFAULT_ADDR)
    }
}

impl Mcp23017Driver {
    /// Create a new generic driver for the specified I2C address.
    pub const fn new(addr: u8) -> Self {
        Self {
            addr,
            olat_a: 0x00,
            olat_b: 0x00,
            iodir_a: 0xFF, // Power-on default: all pins inputs
            iodir_b: 0xFF,
            gppu_a: 0x00, // Power-on default: pull-ups disabled
            gppu_b: 0x00,
        }
    }

    /// Returns the configured I2C address of this expander.
    pub const fn address(&self) -> u8 {
        self.addr
    }

    /// Reset and configure `IOCON`: BANK=0 (sequential registers), SEQOP=0 (auto-increment).
    pub fn init(&mut self, i2c: &mut I2cDriver) -> Result<(), EspError> {
        log::info!("[MCP23017] Initializing generic expander @ 0x{:02X}...", self.addr);
        i2c.write(self.addr, &[regs::IOCON, 0x00], 50)
    }

    /// Configure direction for an entire 8-bit port (1 = Input, 0 = Output).
    pub fn set_port_direction(
        &mut self,
        i2c: &mut I2cDriver,
        port: Port,
        mask: u8,
    ) -> Result<(), EspError> {
        let reg = match port {
            Port::A => {
                self.iodir_a = mask;
                regs::IODIRA
            }
            Port::B => {
                self.iodir_b = mask;
                regs::IODIRB
            }
        };
        i2c.write(self.addr, &[reg, mask], 50)
    }

    /// Configure direction for an individual pin.
    pub fn set_pin_direction(
        &mut self,
        i2c: &mut I2cDriver,
        pin: Pin,
        dir: Direction,
    ) -> Result<(), EspError> {
        let (port, idx) = pin.port_and_index();
        let bit = 1 << idx;
        let mut cur = match port {
            Port::A => self.iodir_a,
            Port::B => self.iodir_b,
        };
        match dir {
            Direction::Input => cur |= bit,
            Direction::Output => cur &= !bit,
        }
        self.set_port_direction(i2c, port, cur)
    }

    /// Configure internal 100k pull-up resistors for an entire 8-bit port (1 = Enabled, 0 = Disabled).
    pub fn set_port_pullup(
        &mut self,
        i2c: &mut I2cDriver,
        port: Port,
        mask: u8,
    ) -> Result<(), EspError> {
        let reg = match port {
            Port::A => {
                self.gppu_a = mask;
                regs::GPPUA
            }
            Port::B => {
                self.gppu_b = mask;
                regs::GPPUB
            }
        };
        i2c.write(self.addr, &[reg, mask], 50)
    }

    /// Configure internal 100k pull-up resistor for an individual pin.
    pub fn set_pin_pullup(
        &mut self,
        i2c: &mut I2cDriver,
        pin: Pin,
        enabled: bool,
    ) -> Result<(), EspError> {
        let (port, idx) = pin.port_and_index();
        let bit = 1 << idx;
        let mut cur = match port {
            Port::A => self.gppu_a,
            Port::B => self.gppu_b,
        };
        if enabled {
            cur |= bit;
        } else {
            cur &= !bit;
        }
        self.set_port_pullup(i2c, port, cur)
    }

    /// Read raw 8-bit input values of an entire port (GPIOA or GPIOB).
    pub fn read_port(&mut self, i2c: &mut I2cDriver, port: Port) -> Result<u8, EspError> {
        let reg = match port {
            Port::A => regs::GPIOA,
            Port::B => regs::GPIOB,
        };
        let mut buf = [0u8; 1];
        i2c.write_read(self.addr, &[reg], &mut buf, 50)?;
        Ok(buf[0])
    }

    /// Write raw 8-bit values to an output latch (OLATA or OLATB).
    pub fn write_port(
        &mut self,
        i2c: &mut I2cDriver,
        port: Port,
        val: u8,
    ) -> Result<(), EspError> {
        let reg = match port {
            Port::A => {
                self.olat_a = val;
                regs::OLATA
            }
            Port::B => {
                self.olat_b = val;
                regs::OLATB
            }
        };
        i2c.write(self.addr, &[reg, val], 50)
    }

    /// Set an individual output pin high or low without extra read-modify-write I2C transactions.
    pub fn write_pin(
        &mut self,
        i2c: &mut I2cDriver,
        pin: Pin,
        high: bool,
    ) -> Result<(), EspError> {
        let (port, idx) = pin.port_and_index();
        let bit = 1 << idx;
        let mut cur = match port {
            Port::A => self.olat_a,
            Port::B => self.olat_b,
        };
        if high {
            cur |= bit;
        } else {
            cur &= !bit;
        }
        self.write_port(i2c, port, cur)
    }

    /// Read an individual input pin.
    pub fn read_pin(&mut self, i2c: &mut I2cDriver, pin: Pin) -> Result<bool, EspError> {
        let (port, idx) = pin.port_and_index();
        let port_val = self.read_port(i2c, port)?;
        Ok((port_val & (1 << idx)) != 0)
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_initial_state() {
        let driver = Mcp23017Driver::new(MCP23017_DEFAULT_ADDR);
        assert_eq!(driver.address(), 0x20);
        assert_eq!(driver.olat_a, 0x00);
        assert_eq!(driver.olat_b, 0x00);
        assert_eq!(driver.iodir_a, 0xFF);
        assert_eq!(driver.iodir_b, 0xFF);
        assert_eq!(driver.gppu_a, 0x00);
        assert_eq!(driver.gppu_b, 0x00);
    }

    #[test]
    fn test_pin_port_and_mask_mapping() {
        assert_eq!(Pin::A0.port_and_index(), (Port::A, 0));
        assert_eq!(Pin::A0.mask(), 0b0000_0001);

        assert_eq!(Pin::A1.port_and_index(), (Port::A, 1));
        assert_eq!(Pin::A1.mask(), 0b0000_0010);

        assert_eq!(Pin::A7.port_and_index(), (Port::A, 7));
        assert_eq!(Pin::A7.mask(), 0b1000_0000);

        assert_eq!(Pin::B0.port_and_index(), (Port::B, 0));
        assert_eq!(Pin::B0.mask(), 0b0000_0001);

        assert_eq!(Pin::B2.port_and_index(), (Port::B, 2));
        assert_eq!(Pin::B2.mask(), 0b0000_0100);

        assert_eq!(Pin::B7.port_and_index(), (Port::B, 7));
        assert_eq!(Pin::B7.mask(), 0b1000_0000);
    }

    #[test]
    fn test_pin_direction_calculation() {
        let mut driver = Mcp23017Driver::new(0x20);
        assert_eq!(driver.iodir_a, 0xFF);

        // Setting A0 to Output should clear bit 0
        let (port, idx) = Pin::A0.port_and_index();
        let bit = 1 << idx;
        driver.iodir_a &= !bit;
        assert_eq!(driver.iodir_a, 0xFE);
        assert_eq!(port, Port::A);

        // Setting A1 to Output should clear bit 1
        let (port, idx) = Pin::A1.port_and_index();
        let bit = 1 << idx;
        driver.iodir_a &= !bit;
        assert_eq!(driver.iodir_a, 0xFC);
        assert_eq!(port, Port::A);
    }

    #[test]
    fn test_output_latch_caching() {
        let mut driver = Mcp23017Driver::new(0x20);
        assert_eq!(driver.olat_a, 0x00);

        // Set A0 high
        let bit = Pin::A0.mask();
        driver.olat_a |= bit;
        assert_eq!(driver.olat_a, 0x01);

        // Set A1 high
        let bit = Pin::A1.mask();
        driver.olat_a |= bit;
        assert_eq!(driver.olat_a, 0x03);

        // Set A0 low
        let bit = Pin::A0.mask();
        driver.olat_a &= !bit;
        assert_eq!(driver.olat_a, 0x02);
    }
}
