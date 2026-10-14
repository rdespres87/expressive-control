# ToF Theremin: Detailed Electrical Netlist & Wiring Guide

This document serves as the master electrical wiring guide (Netlist) for the ToF Theremin project. It details every physical connection required between the Elecrow ESP32-S3 5.0" CrowPanel module, the external MCP23017 I/O expander, the dual VL53L4CD ToF distance sensors, and the auxiliary RP2040-Zero USB-MIDI coprocessor.

## 1. Power Distribution Bus
The entire digital logic operates on **3.3V**. The CrowPanel module's onboard regulator supplies 3.3V from the 5V USB input. **Do not connect 5V to any external logic pins.**

| Source (CrowPanel) | Destination Component | Destination Pin | Notes |
| :--- | :--- | :--- | :--- |
| `3V3` (Expansion / Grove) | Left ToF (VL53L4CD) | `VIN` / `VCC` | Power for Volume Sensor |
| `3V3` (Expansion / Grove) | Right ToF (VL53L4CD) | `VIN` / `VCC` | Power for Pitch Sensor |
| `3V3` (Expansion / Grove) | Waveshare MCP23017 | `VCC` | Power for I/O Expander |
| `3V3` (Expansion / Grove) | RP2040-Zero | `3V3` | Logic power (or 5V to `5V` pin) |
| `GND` (Expansion / Grove) | **All Components** | `GND` | **Common Ground.** All GND pins tied together. |

---

## 2. The Shared I2C Bus (400 kHz Fast Mode)

Operating Frequency: **400 kHz**.  
*Pull-up resistors:* The CrowPanel board and breakout boards include onboard pull-ups. An additional 4.7 kΩ pull-up pair is placed at the expansion junction to maintain sharp rise times over the 25–35 cm sensor arm cables.

| Source (CrowPanel) | Destination Component | Destination Pin | Address | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `GPIO 19` (SDA) | Goodix GT911 | `SDA` | `0x14` / `0x5D` | Onboard Capacitive Touch |
| `GPIO 19` (SDA) | NXP PCA9557 | `SDA` | `0x18` | Onboard IO Expander |
| `GPIO 19` (SDA) | Waveshare MCP23017 | `SDA` | `0x20` | Front-panel & Sensor reset expander |
| `GPIO 19` (SDA) | Left ToF (VL53L4CD) | `SDA` | `0x30` | Left Hand (Volume) — dynamic after boot |
| `GPIO 19` (SDA) | Right ToF (VL53L4CD) | `SDA` | `0x31` | Right Hand (Pitch) — dynamic after boot |
| `GPIO 19` (SDA) | RP2040-Zero | `GPIO 4` (I2C0 SDA) | `0x40` | USB-MIDI Coprocessor Target (Slave) |
| `GPIO 20` (SCL) | Goodix GT911 | `SCL` | `0x14` / `0x5D` | Onboard Capacitive Touch |
| `GPIO 20` (SCL) | NXP PCA9557 | `SCL` | `0x18` | Onboard IO Expander |
| `GPIO 20` (SCL) | Waveshare MCP23017 | `SCL` | `0x20` | Front-panel & Sensor reset expander |
| `GPIO 20` (SCL) | Left ToF (VL53L4CD) | `SCL` | `0x30` | Left Hand (Volume) — dynamic after boot |
| `GPIO 20` (SCL) | Right ToF (VL53L4CD) | `SCL` | `0x31` | Right Hand (Pitch) — dynamic after boot |
| `GPIO 20` (SCL) | RP2040-Zero | `GPIO 5` (I2C0 SCL) | `0x40` | USB-MIDI Coprocessor Target (Slave) |

---

## 3. Sensor Reset & Addressing (via MCP23017)

Both VL53L4CD sensors default to address `0x29` on power-up. Their `XSHUT` reset pins are driven sequentially by the MCP23017 to eliminate address collisions:

| Source (MCP23017) | Destination Component | Destination Pin | Active Level | Function |
| :--- | :--- | :--- | :--- | :--- |
| `GPA0` | Left ToF (VL53L4CD) | `XSHUT` | Active LOW | Re-addresses sensor #1 to `0x30` |
| `GPA1` | Right ToF (VL53L4CD) | `XSHUT` | Active LOW | Re-addresses sensor #2 to `0x31` |

---

## 4. Physical Front-Panel Controls (via MCP23017)

All front-panel physical controls use the internal 100kΩ pull-up resistors of the MCP23017:

| Source (Component) | Destination (MCP23017) | Target Pin | Electrical Type | Function |
| :--- | :--- | :--- | :--- | :--- |
| Rotary Encoder EC11 (`CLK` / A) | MCP23017 Port B | `GPB0` | Input (Pull-up) | Quadrature Phase A |
| Rotary Encoder EC11 (`DT` / B)  | MCP23017 Port B | `GPB1` | Input (Pull-up) | Quadrature Phase B |
| Rotary Encoder EC11 (`SW`)      | MCP23017 Port B | `GPB2` | Input (Pull-up) | Pushbutton click to select |
| Action Button 1 (Preset +)      | MCP23017 Port A | `GPA4` | Input (Pull-up) | Next preset (to GND) |
| Action Button 2 (Preset -)      | MCP23017 Port A | `GPA5` | Input (Pull-up) | Previous preset (to GND) |
| Action Button 3 (Octave Shift)  | MCP23017 Port A | `GPA6` | Input (Pull-up) | Octave jump (to GND) |

---

## 5. Auxiliary USB-MIDI Coprocessor (RP2040-Zero)

The RP2040-Zero operates as an **I2C Target (Slave)** on address `0x40`:

| RP2040 Pin | Function | Connection |
| :--- | :--- | :--- |
| `GPIO 4` | `I2C0 SDA` | Connected to shared I2C bus (`GPIO 19`) |
| `GPIO 5` | `I2C0 SCL` | Connected to shared I2C bus (`GPIO 20`) |
| `USB Type-C` | USB-MIDI Class Compliant | Connected to Host DAW / Synthesizer |
| `GND` | Common Ground | Connected to shared system ground |
