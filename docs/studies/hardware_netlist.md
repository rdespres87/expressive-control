# ToF Theremin: Detailed Electrical Netlist

This document serves as the master electrical wiring guide (Netlist) for the ToF Theremin project. It details every physical connection required between the Elecrow ESP32-S3 5.0" module and the external peripherals.

## 1. Power Distribution Bus
The entire system operates on 3.3V logic. The Elecrow module's internal regulator supplies the 3.3V. **Do not use 5V for any external logic pins.**

| Source (Elecrow) | Destination Component | Destination Pin | Notes |
| :--- | :--- | :--- | :--- |
| `3V3` (Expansion) | Left ToF (VL53L4CD) | `VIN` / `VCC` | Power for Left Sensor |
| `3V3` (Expansion) | Right ToF (VL53L4CD) | `VIN` / `VCC` | Power for Right Sensor |
| `3V3` (Expansion) | ADC (ADS1115) | `VDD` | Power for ADC |
| `3V3` (Expansion) | Encoder (KY-040) | `+` / `VCC` | Power for Encoder pull-ups |
| `3V3` (Expansion) | Potentiometers 1 to 4 | `Pin 1` (CCW) | Provide high reference voltage |
| `GND` (Expansion) | **All Components** | `GND` / `Pin 3` | **Common Ground.** All GND pins must be tied together. |

## 2. The Shared I2C Bus
The I2C bus operates at 400kHz or 1MHz. 
*Note on Pull-up resistors:* The Elecrow board, ADS1115 breakout, and VL53L4CD breakouts typically have built-in 10kΩ pull-up resistors on SDA/SCL. No external resistors are required.

| Source (Elecrow) | Destination Component | Destination Pin | Notes |
| :--- | :--- | :--- | :--- |
| `I2C SDA` | Left ToF (VL53L4CD) | `SDA` | Data line |
| `I2C SDA` | Right ToF (VL53L4CD) | `SDA` | Data line |
| `I2C SDA` | ADC (ADS1115) | `SDA` | Data line |
| `I2C SCL` | Left ToF (VL53L4CD) | `SCL` | Clock line |
| `I2C SCL` | Right ToF (VL53L4CD) | `SCL` | Clock line |
| `I2C SCL` | ADC (ADS1115) | `SCL` | Clock line |

## 3. Digital Logic & I2C Address Management
To prevent the two ToF sensors from crashing the bus (both default to address `0x52`), their hardware reset pins must be controlled by the ESP32.

| Source (Elecrow) | Destination Component | Destination Pin | Notes |
| :--- | :--- | :--- | :--- |
| `GPIO_A` (Any free OUT) | Left ToF (VL53L4CD) | `LPn` / `XSHUT` | Boots Left sensor to change address to 0x54. |
| `GPIO_B` (Any free OUT) | Right ToF (VL53L4CD)| `LPn` / `XSHUT` | Boots Right sensor (stays at 0x52). |
| `GPIO_X` (Any free IN)  | Left ToF (VL53L4CD) | `INT`           | Hardware interrupt for Left sensor (Data Ready). |
| `GPIO_Y` (Any free IN)  | Right ToF (VL53L4CD)| `INT`           | Hardware interrupt for Right sensor (Data Ready). |
| `GND` | ADC (ADS1115) | `ADDR` | Hardwires the ADC I2C address to `0x48`. |

## 4. Analog Controls (Potentiometers)
The potentiometers are read by the ADS1115 to free up ESP32 pins and avoid ESP32 ADC noise.

| Component (Source) | Destination (ADS1115) | Destination Pin | Notes |
| :--- | :--- | :--- | :--- |
| Potentiometer 1 (Vol) | ADS1115 | `A0` | Connect to middle pin (Wiper) of Pot 1 |
| Potentiometer 2 (Pitch)| ADS1115 | `A1` | Connect to middle pin (Wiper) of Pot 2 |
| Potentiometer 3 (Curve)| ADS1115 | `A2` | Connect to middle pin (Wiper) of Pot 3 |
| Potentiometer 4 (Aux) | ADS1115 | `A3` | Connect to middle pin (Wiper) of Pot 4 |

## 5. Digital Inputs (Encoder & Buttons)
The KY-040 module has built-in pull-up resistors for the encoder pins, but not always for the switch. We will rely on ESP32 internal pull-ups for the buttons.

| Source (Component) | Destination (Elecrow) | Dest Pin | Notes |
| :--- | :--- | :--- | :--- |
| Encoder KY-040 (`CLK`) | Elecrow GPIO Header | `GPIO_C` | Rotation Phase A |
| Encoder KY-040 (`DT`)  | Elecrow GPIO Header | `GPIO_D` | Rotation Phase B |
| Encoder KY-040 (`SW`)  | Elecrow GPIO Header | `GPIO_E` | Encoder Push Button (Configured as `INPUT_PULLUP`) |
| Action Button 1        | Elecrow GPIO Header | `GPIO_F` | Push Button. Other pin goes to `GND`. (`INPUT_PULLUP`) |
| Action Button 2        | Elecrow GPIO Header | `GPIO_G` | Push Button. Other pin goes to `GND`. (`INPUT_PULLUP`) |

## 6. Electronics Best Practices for the Final Shield
If soldering a custom PCB "Shield" for the Elecrow:
1.  **Decoupling Capacitors:** Add a `0.1µF` ceramic capacitor close to the power pins of the ADS1115 and each VL53L4CD.
2.  **Hardware Debouncing (Optional):** Add a `10nF` ceramic capacitor between `CLK` and `GND`, and `DT` and `GND` on the encoder to filter electrical noise (though this can be handled via software debouncing).
3.  **Trace routing:** Keep the I2C (`SDA`/`SCL`) traces as short as possible to maintain signal integrity at high speeds.

---

## 7. 🏗 Architecture Study: "Dual-Board" (2x Elecrow Displays)

Using **two** Elecrow 5.0" boards (one for each hand) is an excellent architectural evolution that radically transforms the ergonomics and design of the instrument.

### 7.1 Ergonomic & Aesthetic Benefits
*   **Perfect Symmetry:** Each hand (Pitch on the right, Volume on the left) has its own dedicated 5-inch dashboard.
*   **Specialized Displays:**
    *   **Pitch Screen (Right Hand):** Can display a large "Strobe Tuner", the exact note name, and octave information with fluid animations.
    *   **Volume Screen (Left Hand):** Can display a real-time oscilloscope, dynamic range settings (dB), and system menus.
*   **Physical Modularity:** The two "towers" of the Theremin can be spaced freely according to the musician's morphology, without being constrained by a small central chassis.

### 7.3 Communication & Hardware Impact
*   **Power Supply:** A single Elecrow 5.0" screen draws ~400mA. Two boards will draw ~1A combined (5W). Modern Macs easily supply >30W per port, so a single USB connection to the Master board (which then powers the Slave board via its 5V expansion pins) will work perfectly without brownouts.
*   **Sensor Topology:** To guarantee 0ms synchronization drift between the two hands, **BOTH VL53L4CD ToF sensors must be wired to the Master board**. 
*   **Master-to-Slave Data Link & Safe OTA Update:** The Master and Slave will be connected by a 5-wire harness:
    1.  `GND`: Common ground.
    2.  `Master TX` -> `Slave RX`: For real-time Pitch telemetry and firmware flashing.
    3.  `Master RX` <- `Slave TX`: For Slint UI interactivity (touch screen menus).
    4.  `Master GPIO_A` -> `Slave EN` (Reset): Allows the Master to physically reboot the Slave.
    5.  `Master GPIO_B` -> `Slave BOOT0` (GPIO 0): Allows the Master to force the Slave into ROM Bootloader mode.
    *Note: Controlling `EN` and `BOOT0` guarantees a 100% unbrickable dual-board OTA update architecture using the `esp-serial-flasher` library on the Master.* 

### 7.4 UART "Plug & Play" Architecture
To support seamlessly starting with 1 screen and upgrading to 2 screens later, the firmware implements a "Plug & Play" heartbeat mechanism over UART:
1.  **Autonomous Master:** The Master Board (Volume Hand) reads both sensors, generates USB MIDI, and constantly broadcasts Pitch telemetry on its UART `TX` pin.
2.  **Slave Heartbeat:** If a Slave Board (Pitch Hand) is plugged in, it reads the telemetry to animate the Tuner, and continuously sends a "Ping" (Heartbeat) back to the Master on the Master's `RX` pin.
3.  **Hot-Swap UI:**
    *   When the Master receives the heartbeat, it knows the second screen is connected. It dynamically adapts its Slint UI to hide the Pitch widget and expand the Volume widget (Fullscreen Volume Mode).
    *   If the Master loses the heartbeat for > 2 seconds (cable unplugged), it automatically reverts its UI to the Combined Mode (Pitch + Volume on the single screen).
