# Elecrow CrowPanel 5.0" ESP32-S3 V3.0 — LCD Integration

## Overview

This document describes the integration of the 800×480 RGB LCD display on the Elecrow CrowPanel 5.0" ESP32-S3 V3.0 board into the expressive-control firmware.

## Hardware Reference

- **Schematic**: [`Eagle_SCH&PCB/V3.0/CrowPanel ESP32 Display-5.0-inch-V3.0-20240314 .sch`](https://github.com/Elecrow-RD/CrowPanel-5.0-HMI-ESP32-Display-800x480/blob/master/Eagle_SCH%26PCB/V3.0/CrowPanel%20ESP32%20Display-5.0-inch-V3.0-20240314%20.sch)
- **PDF**: [`Eagle_SCH&PCB/V3.0/CrowPanel ESP32 Display-5.0-inch-V3.0-20240314 .pdf`](https://github.com/Elecrow-RD/CrowPanel-5.0-HMI-ESP32-Display-800x480/blob/master/Eagle_SCH%26PCB/V3.0/CrowPanel%20ESP32%20Display-5.0-inch-V3.0-20240314%20.pdf)
- **Product Guide**: [`Eagle_SCH&PCB/V3.0/readme.md`](https://github.com/Elecrow-RD/CrowPanel-5.0-HMI-ESP32-Display-800x480/blob/master/Eagle_SCH%26PCB/V3.0/readme.md)

## Display Specifications

| Parameter | Value |
|-----------|-------|
| Panel | 5.0" IPS LCD |
| Resolution | 800 × 480 |
| Color Format | RGB565 (16-bit) |
| Interface | 16-bit RGB parallel (LCD_CAM controller) |
| Pixel Clock | 16 MHz |
| Controller | ESP32-S3 LCD_CAM (direct RGB output) |

## Pin Mapping

### LCD Data Pins (16-bit RGB565)

| Pin | GPIO | Description |
|-----|------|-------------|
| D0 | 8 | Red bit 0 |
| D1 | 3 | Red bit 1 |
| D2 | 46 | Red bit 2 |
| D3 | 9 | Red bit 3 |
| D4 | 1 | Red bit 4 |
| D5 | 5 | Green bit 0 |
| D6 | 6 | Green bit 1 |
| D7 | 7 | Green bit 2 |
| D8 | 15 | Green bit 3 |
| D9 | 16 | Green bit 4 |
| D10 | 4 | Green bit 5 |
| D11 | 45 | Blue bit 0 |
| D12 | 48 | Blue bit 1 |
| D13 | 47 | Blue bit 2 |
| D14 | 21 | Blue bit 3 |
| D15 | 14 | Blue bit 4 |

### LCD Timing Signals

| Signal | GPIO | Description |
|--------|------|-------------|
| HSYNC | 39 | Horizontal sync |
| VSYNC | 41 | Vertical sync |
| DE | 40 | Data Enable |
| PCLK | 0 | Pixel clock |

### Backlight

| Signal | GPIO | Description |
|--------|------|-------------|
| EN | 2 | MT9201 boost converter enable (active high) |

## Timing Parameters

| Parameter | Value |
|-----------|-------|
| H_RES | 800 |
| V_RES | 480 |
| HSYNC Pulse Width | 48 |
| HSYNC Back Porch | 40 |
| HSYNC Front Porch | 48 |
| VSYNC Pulse Width | 3 |
| VSYNC Back Porch | 13 |
| VSYNC Front Porch | 3 |
| PCLK | 16 MHz |

## Implementation Files

### [`firmwares/expressive-theremin/src/platform.rs`](../../firmwares/expressive-theremin/src/platform.rs)

The LCD driver and platform abstraction layer:

- `LcdDriver::new()` — Initializes the RGB panel via `esp_lcd_new_rgb_panel()`
- `init_lcd()` — Global initialization function (called from main.rs)
- `flush_frame()` — Flushes dirty regions to the LCD
- `mark_dirty(x, y, w, h)` — Marks a region as needing update
- `FrameBuffer` — Manages dual RGB565 buffers in PSRAM

### [`firmwares/expressive-theremin/src/main.rs`](../../firmwares/expressive-theremin/src/main.rs)

- Imports `platform` module
- Calls `init_lcd()` before Slint UI initialization
- Calls `flush_frame()` in the main loop after each sensor cycle

### [`firmwares/expressive-theremin/sdkconfig.defaults`](../../firmwares/expressive-theremin/sdkconfig.defaults)

- `CONFIG_SPIRAM=y` — Enable PSRAM
- `CONFIG_SPIRAM_MODE_OPI=y` — OPI mode (4-bit wide, required for high bandwidth)
- `CONFIG_SPIRAM_SPEED_80M=y` — 80 MHz PSRAM speed

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     ESP32-S3 WROOM                          │
│                                                             │
│  ┌──────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │ Slint UI │───▶│ Frame Buffer │───▶│ LCD_CAM Peripheral│──▶ LCD Panel
│  │ (software│    │ (PSRAM, 2×   │    │ (RGB parallel    │    800×480
│  │ renderer)│    │  768 KB)     │    │  16-bit)         │    RGB565
│  └──────────┘    └──────────────┘    └──────────────────┘  │
│                            ▲                                  │
│                            │                                 │
│                    heap_caps_malloc                         │
│                    (MALLOC_CAP_SPIRAM)                      │
└─────────────────────────────────────────────────────────────┘
```

## Memory Requirements

| Buffer | Size | Location |
|--------|------|----------|
| Front buffer | 800 × 480 × 2 = 768 KB | PSRAM (OPI) |
| Back buffer | 800 × 480 × 2 = 768 KB | PSRAM (OPI) |
| **Total** | **1.5 MB** | PSRAM required |

## Build Configuration

The firmware must be built with:
1. ESP-IDF 5.x (for `esp_lcd` RGB panel support)
2. OPI PSRAM enabled in sdkconfig
3. `esp-idf-sys` 0.38+ for FFI bindings

## Testing Checklist

- [ ] LCD backlight turns on (GPIO2 HIGH)
- [ ] Black screen displayed after initialization
- [ ] Slint UI renders correctly
- [ ] No flickering or artifacts
- [ ] PSRAM allocation succeeds (check logs)
- [ ] DMA transfers complete without errors

## Known Issues

1. **PSRAM must be OPI mode** — QPI mode won't provide enough bandwidth for dual buffering
2. **GPIO0 contention** — GPIO0 is shared with LCD PCLK; do not use as GPIO
3. **No touch integration yet** — GT911 capacitive touch (I2C via PCA9557) is not yet integrated
4. **Full frame flush** — Currently flushes entire dirty region; could be optimized for partial updates

## Bug Fix: Struct Layout Mismatch (2026-10-09)

**Problem**: The error `unsupported data width 13` occurred because the Rust struct `RgbPanelConfig` had incorrect field ordering compared to ESP-IDF's `esp_lcd_rgb_panel_config_t`.

**Root cause**: The original struct had extra fields (`pclk_hz`, `trans_queue_depth`, `resolution`) before `timings`, shifting all subsequent fields. At offset 44, ESP-IDF read `timings.vsync_back_porch` (value 13) as `data_width`, hence "unsupported data width 13".

**Fix**: Realigned the struct to match ESP-IDF 5.2.3 exactly:
- `clk_src` (4 bytes) → `timings` (40 bytes) → `data_width` (4 bytes) → ...
- Replaced `DataGpio` struct with `[i32; 16]` array for `data_gpio_nums`
- Added proper `bits_per_pixel`, `num_fbs`, `bounce_buffer_size_px`, alignment fields
- Set `FB_IN_PSRAM` flag to allocate frame buffer in PSRAM

## Future Work

1. Implement Slint `Platform` trait properly for direct frame buffer integration
2. Add touch input support (GT911 via PCA9557 I2C expander)
3. Optimize dirty region tracking for Slint render callbacks
4. Add double-buffer swapping for tear-free updates
5. Support for partial screen updates to reduce DMA traffic

## References

- [ESP-IDF esp_lcd Component](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-reference/peripherals/lcd.html)
- [ESP32-S3 Technical Reference — LCD_CAM](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/hw-reference/constraint-and-parameters.html#lcd-cam)
- [Slint Rust Embedded Documentation](https://slint.dev/releases/1.18/docs/rust/slint/)
- [Elecrow CrowPanel Repository](https://github.com/Elecrow-RD/CrowPanel-5.0-HMI-ESP32-Display-800x480)
