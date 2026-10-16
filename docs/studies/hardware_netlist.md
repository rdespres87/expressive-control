# ToF Theremin: Detailed Electrical Netlist & Wiring Guide

This document serves as the master electrical wiring guide (Netlist) for the ToF Theremin project. It details every physical connection required between the Elecrow ESP32-S3 5.0" CrowPanel module, the dual VL53L4CD ToF distance sensors, and the auxiliary RP2040-Zero Coprocessor (which centralizes all front-panel buttons, rotary encoder, and ToF reset lines).

## 1. Power Distribution Bus
The entire digital logic operates on **3.3V**. The CrowPanel module's onboard regulator supplies 3.3V from the 5V USB input. **Do not connect 5V to any external logic pins.**

| Source (CrowPanel) | Destination Component | Destination Pin | Notes |
| :--- | :--- | :--- | :--- |
| `3V3` (Expansion / Grove) | Left ToF (VL53L4CD) | `VIN` / `VCC` | Power for Volume Sensor |
| `3V3` (Expansion / Grove) | Right ToF (VL53L4CD) | `VIN` / `VCC` | Power for Pitch Sensor |
| `3V3` (Expansion / Grove) | RP2040-Zero | `3V3` | Logic power (3.3V logic supply) |
| `GND` (Expansion / Grove) | **All Components** | `GND` | **Common Ground.** All GND pins tied together. |

---

## 2. The Shared I2C Bus (400 kHz Fast Mode)

Operating Frequency: **400 kHz**.  
*Pull-up resistors:* The CrowPanel board and breakout boards include onboard pull-ups.

| Source (CrowPanel) | Destination Component | Destination Pin | Address | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `GPIO 19` (SDA) | Goodix GT911 | `SDA` | `0x14` / `0x5D` | Onboard Capacitive Touch |
| `GPIO 19` (SDA) | NXP PCA9557 | `SDA` | `0x18` | Onboard IO Expander |
| `GPIO 19` (SDA) | Left ToF (VL53L4CD) | `SDA` | `0x30` | Left Hand (Volume) — dynamic after boot |
| `GPIO 19` (SDA) | Right ToF (VL53L4CD) | `SDA` | `0x31` | Right Hand (Pitch) — dynamic after boot |
| `GPIO 19` (SDA) | RP2040-Zero | `GP0` (I2C0 SDA) | `0x40` | Front-Panel & USB-MIDI Coprocessor Target |
| `GPIO 20` (SCL) | Goodix GT911 | `SCL` | `0x14` / `0x5D` | Onboard Capacitive Touch |
| `GPIO 20` (SCL) | NXP PCA9557 | `SCL` | `0x18` | Onboard IO Expander |
| `GPIO 20` (SCL) | Left ToF (VL53L4CD) | `SCL` | `0x30` | Left Hand (Volume) — dynamic after boot |
| `GPIO 20` (SCL) | Right ToF (VL53L4CD) | `SCL` | `0x31` | Right Hand (Pitch) — dynamic after boot |
| `GPIO 20` (SCL) | RP2040-Zero | `GP1` (I2C0 SCL) | `0x40` | Front-Panel & USB-MIDI Coprocessor Target |

*(Note: The MCP23017 expander at `0x20` has been eliminated. All front-panel controls and ToF reset lines are managed directly by the RP2040-Zero).*

---

## 3. Sensor Reset & Addressing (via RP2040-Zero)

Both VL53L4CD sensors default to address `0x29` on power-up. Their `XSHUT` reset pins are driven sequentially by the RP2040-Zero via I2C command `0x20` to eliminate address collisions:

| Source (RP2040-Zero) | Destination Component | Destination Pin | Active Level | Function |
| :--- | :--- | :--- | :--- | :--- |
| `GP2` | Left ToF (VL53L4CD) | `XSHUT` | Active LOW (Push-Pull) | Re-addresses sensor #1 to `0x30` |
| `GP3` | Right ToF (VL53L4CD) | `XSHUT` | Active LOW (Push-Pull) | Re-addresses sensor #2 to `0x31` |

---

## 4. Physical Front-Panel Controls (via RP2040-Zero)

All front-panel physical controls use the internal pull-up resistors of the RP2040-Zero:

| Source (Component) | Destination (RP2040-Zero) | Target Pin | Electrical Type | Function |
| :--- | :--- | :--- | :--- | :--- |
| Rotary Encoder EC11 (`CLK` / A) | RP2040-Zero | `GP6` | Input (Pull-up) | Quadrature Phase A (sub-microsecond decoding) |
| Rotary Encoder EC11 (`DT` / B)  | RP2040-Zero | `GP7` | Input (Pull-up) | Quadrature Phase B (sub-microsecond decoding) |
| Rotary Encoder EC11 (`SW`)      | RP2040-Zero | `GP8` | Input (Pull-up) | Pushbutton click to toggle UI View / Select (to GND) |
| Action Button 1 (Octave +)      | RP2040-Zero | `GP14`| Input (Pull-up) | Increment base octave (to GND) |
| Action Button 2 (Octave -)      | RP2040-Zero | `GP15`| Input (Pull-up) | Decrement base octave (to GND) |
| Action Button 3 (Mute)          | RP2040-Zero | `GP26`| Input (Pull-up) | Toggle audio output mute (to GND) |

---

## 5. Auxiliary Front-Panel & USB-MIDI Coprocessor (RP2040-Zero)

The RP2040-Zero operates as an **I2C Target (Slave)** on address `0x40`:

| RP2040 Pin | Function | Connection |
| :--- | :--- | :--- |
| `GP0` | `I2C0 SDA` | Connected to shared I2C bus (`GPIO 19`) |
| `GP1` | `I2C0 SCL` | Connected to shared I2C bus (`GPIO 20`) |
| `GP2` | `XSHUT_LEFT` | Left ToF (Volume) Shutdown line |
| `GP3` | `XSHUT_RIGHT` | Right ToF (Pitch) Shutdown line |
| `GP4` | `UART1 TX` | FTDI Serial Console RX (115200 baud) |
| `GP5` | `UART1 RX` | FTDI Serial Console TX (115200 baud) |
| `GP6` | `ENCODER_CLK` | Rotary Encoder Phase A |
| `GP7` | `ENCODER_DT` | Rotary Encoder Phase B |
| `GP8` | `ENCODER_SW` | Rotary Encoder Push Switch |
| `GP14` | `BTN_OCT_UP` | Front-panel Octave + button |
| `GP15` | `BTN_OCT_DOWN` | Front-panel Octave - button |
| `GP26` | `BTN_MUTE` | Front-panel Mute button |
| `GP16` | `WS2812` | Onboard RGB Status LED |
| `USB Type-C` | USB-MIDI Class Compliant | Connected to Host DAW / Synthesizer |
| `GND` | Common Ground | Connected to shared system ground |
| `3V3` | Power Input | 3.3V Logic Supply |
