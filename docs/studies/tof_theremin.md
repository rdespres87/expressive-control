# ToF Theremin Feasibility and Design Study

## 1. Overview
The goal of this project is to develop a modern, digital, MIDI-based Theremin using Time-of-Flight (ToF) sensors, supporting MPE, and utilizing a 5" to 6" OLED/LCD touch display for configuration.

Based on hardware evaluations, the **ESP32-S3** has been selected as the primary Microcontroller (MCU). This allows a "Single-MCU" architecture that can drive a large high-resolution GUI natively while handling USB MIDI and sensor polling.

## 2. Hardware Architecture & Bill of Materials (BOM)

### 2.1 Main MCU & Display Controller
**Recommendation: ESP32-S3 Integrated Display Boards**
Instead of building a custom PCB immediately, the most efficient path for development is using an off-the-shelf ESP32-S3 board with an integrated 5" capacitive touch screen. The ESP32-S3 provides native USB OTG (required for USB-MIDI), high-speed I2C/SPI, and an RGB LCD peripheral.

*   **Reference 1: Elecrow CrowPanel ESP32 Display 5.0"**
    *   *Specs:* ESP32-S3, 800x480 TFT LCD, Capacitive Touch, RGB565 interface.
    *   *Why:* Extremely popular, great documentation, exposes plenty of GPIOs via external headers (crucial for I2C sensors and potentiometers). It usually features a native USB-C port for USB-MIDI support.
*   **Reference 2: Sunton ESP32-8048S050C (Yellow Board)**
    *   *Specs:* ESP32-S3, 5.0" 800x480 Capacitive Touch.
    *   *Why:* Highly cost-effective (~$30-40) and heavily supported by the open-source community. 
    *   *Warning:* Ensure the native USB (GPIO 19/20) is exposed or wired to a USB-C connector for MIDI Device capability, as the primary USB port is often just for flashing (CH340/UART).

### 2.2 Time-of-Flight (ToF) Sensors
The choice of sensor defines the instrument's capabilities. Two sensors are needed (Left Hand = Volume, Right Hand = Pitch).

*   **Option A: STMicroelectronics VL53L5CX (Multizone - Recommended for MPE)**
    *   *Features:* 8x8 multizone matrix, up to 60Hz refresh rate, 4-meter range.
    *   *Why:* Allows 3D tracking. You don't just get the Z distance (Pitch), you get the X/Y position of the hand within the cone. This allows mapping horizontal/vertical hand tilt to **CC4 (Timbre)** or **CC1 (Modulation)**, unlocking true MPE.
    *   *Breakout Board References:* 
        *   SparkFun Qwiic Multi Distance Sensor - VL53L5CX
        *   Adafruit VL53L5CX Time of Flight 8x8 Matrix Sensor
        *   Pololu VL53L5CX Time-of-Flight 8x8-Zone Distance Sensor Carrier
*   **Option B: STMicroelectronics VL53L4CD (Single Zone - Recommended for Lowest Latency)**
    *   *Features:* Single zone, up to 100Hz refresh rate, 1.3-meter range.
    *   *Why:* If the Theremin feels "sluggish" at 60Hz, this sensor bumps the speed to 100Hz (10ms latency), making it feel incredibly responsive, much closer to a pure analog oscillator. The tradeoff is losing the X/Y axis (no 3D MPE, only strict Z-distance).
    *   *Breakout Board References:* Adafruit VL53L4CD, SparkFun Distance Sensor - VL53L4CD.

*Note on I2C:* Both sensors must share the same I2C bus. You **must** wire the `XSHUT` or `LPn` (Enable) pin of each sensor to two separate GPIOs on the ESP32-S3 to initialize them one by one and change the I2C address of the second sensor at boot.

### 2.3 Additional Controls
*   **Potentiometers:** Standard 10kΩ Linear (Type B) potentiometers (e.g., Bourns PTV09 or Alpha rotary pots). Used for on-the-fly tuning of Pitch scale, Volume threshold, or curve response without touching the screen.
*   **Rotary Encoders:** EC11 with push-button. Excellent for scrolling through menus on the screen precisely.
*   **I/O Expansion (If needed):** If the ESP32-S3 display board does not expose enough Analog/ADC pins for the potentiometers, use an I2C ADC like the **ADS1115** (16-bit, 4 channels) to read the knobs precisely.

## 3. Firmware Integration details (ESP32-S3 in Rust)
Moving to ESP32-S3 changes the firmware stack slightly from the `nRF5340`:
1.  **Framework:** `esp-hal` (no_std) or `esp-idf-hal` (std/FreeRTOS). For complex UI, `esp-idf` is often preferred as it allows utilizing standard libraries and threads.
2.  **UI Library:** **Slint UI** (`slint.dev`). It has first-class support for ESP32 and Rust, allowing you to design the UI in a declarative language (`.slint`) and compile it to native Rust code.
3.  **USB MIDI:** The ESP32-S3 has a native USB OTG peripheral. Using the `usb-device` and `usbd-midi` crates over `esp-hal` allows it to appear as a standard MIDI keyboard to the host PC.
