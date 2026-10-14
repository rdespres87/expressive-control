# Architecture Blueprint: Generic MCP23017 Driver & Composable Peripheral Subsystem

## 1. Problem Statement & User Critique
The initial implementation directly coupled the MCP23017 expander driver to the Theremin application, hardcoding ToF reset lines and front-panel button decoding inside the low-level chip driver.

An I/O expander is a generic hardware component (16 general-purpose digital I/O pins across Port A and Port B). It must not contain application-specific business logic. High-level peripherals (buttons, rotary encoders, ToF reset sequencers) must be standalone, composable modules that consume generic pins from the expander or from any other digital pin source.

---

## 2. Layered Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                      APPLICATION LAYER (Theremin)                      │
│                                                                        │
│   ThereminFrontPanel                 DualTofResetController            │
│   (Polls I2C, dispatches events)     (Orchestrates sequential boot)    │
└───────────────────────┬──────────────────────────┬─────────────────────┘
                        │                          │
                        ▼                          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                     COMPOSABLE PERIPHERAL DRIVERS                      │
│                                                                        │
│   DebouncedButton (Generic)             RotaryEncoder (Generic)        │
│   - Configurable 3-sample integrator    - Gray-code quadrature decoder │
│   - Edge triggers (just_pressed/rel)    - Detent accumulator (EC11)    │
│   - Pure logic (agnostic of pin source) - Pure logic (agnostic of pin) │
└──────────────────────────────────────┬─────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        GENERIC MCP23017 DRIVER                         │
│                                                                        │
│   Mcp23017Driver                                                       │
│   ├── Complete 16-bit register map (IODIR, GPPU, IPOL, GPIO, OLAT...)  │
│   ├── Port-level API: read_port(Port), write_port(Port, val)          │
│   ├── Pin-level API: set_pin(Pin, bool), read_pin(Pin)                │
│   ├── Direction & Pull-up configuration per pin / per port             │
│   └── Cached output latch tracking (zero redundant read-modify-writes) │
└──────────────────────────────────────┬─────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       I2C COMMUNICATION LAYER                          │
│   esp_idf_hal::i2c::I2cDriver  /  MockI2cBus (for host unit tests)     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Component Breakdown

### 3.1 Generic `Mcp23017` Driver (`mcp23017/driver.rs`)
* **Zero application logic:** Has no knowledge of ToF sensors, theremins, or encoders.
* **Pin & Port Enumerations:**
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Port {
      A,
      B,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Pin {
      A0, A1, A2, A3, A4, A5, A6, A7,
      B0, B1, B2, B3, B4, B5, B6, B7,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Direction {
      Input,
      Output,
  }
  ```
* **Core API:**
  - `new(i2c_addr: u8) -> Self`
  - `init(&mut self, i2c: &mut I2cDriver) -> Result<(), EspError>`: resets and configures `IOCON` (`BANK=0`, `SEQOP=0`).
  - `set_port_direction(&mut self, i2c: &mut I2cDriver, port: Port, mask: u8) -> Result<(), EspError>`
  - `set_pin_direction(&mut self, i2c: &mut I2cDriver, pin: Pin, dir: Direction) -> Result<(), EspError>`
  - `set_port_pullups(&mut self, i2c: &mut I2cDriver, port: Port, mask: u8) -> Result<(), EspError>`
  - `set_pin_pullup(&mut self, i2c: &mut I2cDriver, pin: Pin, enabled: bool) -> Result<(), EspError>`
  - `write_port(&mut self, i2c: &mut I2cDriver, port: Port, value: u8) -> Result<(), EspError>`
  - `read_port(&mut self, i2c: &mut I2cDriver, port: Port) -> Result<u8, EspError>`
  - `set_pin(&mut self, i2c: &mut I2cDriver, pin: Pin, high: bool) -> Result<(), EspError>`
  - `read_pin(&mut self, i2c: &mut I2cDriver, pin: Pin) -> Result<bool, EspError>`

### 3.2 Composable `DebouncedButton` (`mcp23017/debounce.rs`)
* Generic 3-sample integrator debouncer.
* Agnostic of physical source: works with MCU pins, expanders, or simulation inputs.
* Exposes `is_pressed`, `just_pressed`, and `just_released`.

### 3.3 Composable `RotaryEncoder` (`mcp23017/encoder.rs`)
* Generic 2-bit Gray-code quadrature decoder.
* Full detent filtering: accumulates 4 fractional transitions per physical notch (EC11 standard).
* Rejects illegal state jumps (contact bounce / chatter).

### 3.4 Composable `DualTofResetController` (`mcp23017/tof_reset.rs`)
* Encapsulates the multi-stage hardware power-on sequence for dual VL53L4CD sensors:
  1. Hold both sensors in hardware shutdown (`XSHUT_LEFT = 0`, `XSHUT_RIGHT = 0`).
  2. Delay 10 ms.
  3. Release Left ToF (`XSHUT_LEFT = 1`) $\to$ wait 5 ms boot $\to$ caller re-addresses from default `0x29` to `0x30`.
  4. Release Right ToF (`XSHUT_RIGHT = 1`) $\to$ wait 5 ms boot $\to$ caller re-addresses from default `0x29` to `0x31`.

### 3.5 Application Front-Panel Assembly (`mcp23017/front_panel.rs`)
* Glues the components together for the Theremin:
  - Button 1: Preset Up (`GPA4`)
  - Button 2: Preset Down (`GPA5`)
  - Button 3: Octave Shift (`GPA6`)
  - Rotary Encoder: CLK (`GPB0`), DT (`GPB1`), Pushbutton (`GPB2`)
  - ToF Reset Lines: Volume (`GPA0`), Pitch (`GPA1`)
* Performs burst reads of Port A and Port B in a single atomic I2C transaction to minimize bus traffic.

---

## 4. Test Strategy
1. **Generic MCP23017 Register & Pin Math:** Unit tests for pin-to-port mapping, bitmask setting/clearing, latch caching.
2. **Generic RotaryEncoder:** Unit tests for full CW/CCW 4-step cycles, glitch rejection, partial steps.
3. **Generic DebouncedButton:** Unit tests for noise filtering, sustained press, clean release edges.
4. **Integration Build & Hardware Check:** `cargo clippy --release --features demo -- -D warnings`.
