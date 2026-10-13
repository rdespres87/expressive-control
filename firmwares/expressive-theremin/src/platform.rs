//! Slint Platform implementation for the Elecrow CrowPanel 5.0" ESP32-S3 V3.0.
//!
//! Drives the 800×480 RGB LCD panel using native ESP-IDF `esp_lcd` with:
//! - 2 PSRAM frame buffers (Double Buffering)
//! - DRAM bounce buffer for smooth DMA transfers
//! - VSYNC callback synchronization
//! - Slint MinimalSoftwareWindow with SwappedBuffers

#![allow(dead_code)]

use crate::MainWindow;
use core::ffi::c_void;
use std::rc::Rc;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use esp_idf_sys::{
    esp_lcd_new_rgb_panel,
    esp_lcd_panel_del,
    esp_lcd_panel_disp_on_off,
    esp_lcd_panel_draw_bitmap,
    esp_lcd_panel_handle_t,
    esp_lcd_panel_init,
    esp_lcd_rgb_panel_config_t,
    esp_lcd_rgb_panel_config_t__bindgen_ty_1,
    esp_lcd_rgb_panel_event_callbacks_t,
    esp_lcd_rgb_panel_event_data_t,
    esp_lcd_rgb_panel_get_frame_buffer,
    esp_lcd_rgb_panel_register_event_callbacks,
    esp_lcd_rgb_timing_t,
    esp_lcd_rgb_timing_t__bindgen_ty_1,
    soc_module_clk_t_SOC_MOD_CLK_PLL_F160M,
    ESP_OK,
};

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::PhysicalSize;

// ─── Sensor state shared between sensor and UI threads ──────────────────────

#[derive(Clone, Default, Debug)]
pub struct SensorState {
    pub pitch_distance: u16,
    pub volume_distance: u16,
    pub is_playing: bool,
    pub pitch_bend: u16,
    pub base_note: u8,
}

static G_SENSOR_STATE: LazyLock<Mutex<SensorState>> =
    LazyLock::new(|| Mutex::new(SensorState::default()));

pub static G_WINDOW_WEAK: LazyLock<Mutex<Option<slint::Weak<MainWindow>>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn update_sensor_state(state: SensorState) {
    let mut guard = G_SENSOR_STATE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = state;
}

pub fn poll_ui_updates(_window_weak: &slint::Weak<MainWindow>) {
    #[allow(unused_variables)]
    let state = {
        let guard = G_SENSOR_STATE.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    log::trace!(
        "[poll_ui_updates] Sensor state: pitch_dist={}, vol_dist={}, is_playing={}",
        state.pitch_distance,
        state.volume_distance,
        state.is_playing
    );
}

// ─── Hardware pin and timing configuration ──────────────────────────────────

pub const H_RES: usize = 800;
pub const V_RES: usize = 480;

const LCD_PCLK_HZ: u32 = 24_000_000;

// Elecrow CrowPanel 5.0" V3.0 Pinout
const LCD_D0: i32 = 8;
const LCD_D1: i32 = 3;
const LCD_D2: i32 = 46;
const LCD_D3: i32 = 9;
const LCD_D4: i32 = 1;
const LCD_D5: i32 = 5;
const LCD_D6: i32 = 6;
const LCD_D7: i32 = 7;
const LCD_D8: i32 = 15;
const LCD_D9: i32 = 16;
const LCD_D10: i32 = 4;
const LCD_D11: i32 = 45;
const LCD_D12: i32 = 48;
const LCD_D13: i32 = 47;
const LCD_D14: i32 = 21;
const LCD_D15: i32 = 14;

const LCD_HSYNC: i32 = 39;
const LCD_VSYNC: i32 = 41;
const LCD_DE: i32 = 40;
const LCD_PCLK_PIN: i32 = 0;
const LCD_BACKLIGHT: i32 = 2;

// Timing parameters (CrowPanel 5.0" 800×480 official LovyanGFX configuration)
const HSYNC_PULSE_WIDTH: u32 = 4;
const HSYNC_BACK_PORCH: u32 = 43;
const HSYNC_FRONT_PORCH: u32 = 8;
const VSYNC_PULSE_WIDTH: u32 = 4;
const VSYNC_BACK_PORCH: u32 = 12;
const VSYNC_FRONT_PORCH: u32 = 8;

// ─── VSYNC Hardware Callback ────────────────────────────────────────────────

static VSYNC: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

unsafe extern "C" fn vsync_callback(
    _panel: esp_lcd_panel_handle_t,
    _edata: *const esp_lcd_rgb_panel_event_data_t,
    _user_ctx: *mut c_void,
) -> bool {
    VSYNC.store(true, core::sync::atomic::Ordering::SeqCst);
    false
}

// ─── LCD Driver ─────────────────────────────────────────────────────────────

pub struct LcdDriver {
    panel: esp_lcd_panel_handle_t,
    buf1: &'static mut [Rgb565Pixel],
    buf2: &'static mut [Rgb565Pixel],
    use_buf1: bool,
}

unsafe impl Send for LcdDriver {}

impl LcdDriver {
    pub fn new() -> Option<Self> {
        log::info!("=== LcdDriver::new() START (Double Buffer + 20-line Bounce Buffer + VSYNC) ===");

        let mut timing_flags = esp_lcd_rgb_timing_t__bindgen_ty_1::default();
        timing_flags.set_pclk_active_neg(1);

        let timings = esp_lcd_rgb_timing_t {
            pclk_hz: LCD_PCLK_HZ,
            h_res: H_RES as u32,
            v_res: V_RES as u32,
            hsync_pulse_width: HSYNC_PULSE_WIDTH,
            hsync_back_porch: HSYNC_BACK_PORCH,
            hsync_front_porch: HSYNC_FRONT_PORCH,
            vsync_pulse_width: VSYNC_PULSE_WIDTH,
            vsync_back_porch: VSYNC_BACK_PORCH,
            vsync_front_porch: VSYNC_FRONT_PORCH,
            flags: timing_flags,
        };

        let mut config_flags = esp_lcd_rgb_panel_config_t__bindgen_ty_1::default();
        config_flags.set_fb_in_psram(1);

        let data_gpio_nums = [
            LCD_D0, LCD_D1, LCD_D2, LCD_D3, LCD_D4, LCD_D5, LCD_D6, LCD_D7,
            LCD_D8, LCD_D9, LCD_D10, LCD_D11, LCD_D12, LCD_D13, LCD_D14, LCD_D15,
        ];

        let config = esp_lcd_rgb_panel_config_t {
            clk_src: soc_module_clk_t_SOC_MOD_CLK_PLL_F160M,
            timings,
            data_width: 16,
            bits_per_pixel: 16,
            num_fbs: 2, // Double buffering in PSRAM
            bounce_buffer_size_px: H_RES * 20, // 20 lines in internal DRAM (32KB) prevents DMA starvation
            sram_trans_align: 4,
            psram_trans_align: 64,
            hsync_gpio_num: LCD_HSYNC,
            vsync_gpio_num: LCD_VSYNC,
            de_gpio_num: LCD_DE,
            pclk_gpio_num: LCD_PCLK_PIN,
            disp_gpio_num: -1, // Elecrow 5.0" has no DISP pin; backlight is MT9201 EN on GPIO2
            data_gpio_nums,
            flags: config_flags,
        };

        let mut panel: esp_lcd_panel_handle_t = core::ptr::null_mut();
        log::info!("Calling esp_lcd_new_rgb_panel...");
        let ret = unsafe { esp_lcd_new_rgb_panel(&config, &mut panel) };
        if ret != ESP_OK || panel.is_null() {
            log::error!("esp_lcd_new_rgb_panel FAILED: ret={}, panel={:?}", ret, panel);
            return None;
        }
        log::info!("esp_lcd_new_rgb_panel SUCCESS, panel={:?}", panel);

        let ret = unsafe { esp_lcd_panel_init(panel) };
        if ret != ESP_OK {
            log::warn!("esp_lcd_panel_init returned {}", ret);
        } else {
            log::info!("esp_lcd_panel_init SUCCESS");
        }

        // Initialize GPIO2 as strong push-pull output, initially OFF (LOW) to avoid flash at boot
        unsafe {
            esp_idf_sys::gpio_reset_pin(LCD_BACKLIGHT);
            esp_idf_sys::gpio_set_direction(LCD_BACKLIGHT, esp_idf_sys::gpio_mode_t_GPIO_MODE_OUTPUT);
            esp_idf_sys::gpio_set_drive_capability(LCD_BACKLIGHT, esp_idf_sys::gpio_drive_cap_t_GPIO_DRIVE_CAP_3);
            esp_idf_sys::gpio_set_level(LCD_BACKLIGHT, 0);
        }
        log::info!("Backlight initialized (GPIO2 LOW, held off until UI renders)");

        // Register VSYNC callback
        let cbs = esp_lcd_rgb_panel_event_callbacks_t {
            on_vsync: Some(vsync_callback),
            ..Default::default()
        };
        let ret = unsafe {
            esp_lcd_rgb_panel_register_event_callbacks(panel, &cbs, core::ptr::null_mut())
        };
        if ret != ESP_OK {
            log::warn!("esp_lcd_rgb_panel_register_event_callbacks failed: {}", ret);
        } else {
            log::info!("VSYNC callback registered");
        }

        // Get both PSRAM frame buffer pointers
        let mut b1: *mut c_void = core::ptr::null_mut();
        let mut b2: *mut c_void = core::ptr::null_mut();
        let ret = unsafe { esp_lcd_rgb_panel_get_frame_buffer(panel, 2, &mut b1, &mut b2) };
        if ret != ESP_OK || b1.is_null() || b2.is_null() {
            log::error!("esp_lcd_rgb_panel_get_frame_buffer failed: {}", ret);
            return None;
        }

        log::info!(
            "Double frame buffers allocated: fb1={:?}, fb2={:?}",
            b1,
            b2
        );

        let pixel_count = H_RES * V_RES;
        unsafe {
            core::ptr::write_bytes(b1 as *mut u8, 0, pixel_count * 2);
            core::ptr::write_bytes(b2 as *mut u8, 0, pixel_count * 2);
        }

        let buf1 = unsafe { core::slice::from_raw_parts_mut(b1 as *mut Rgb565Pixel, pixel_count) };
        let buf2 = unsafe { core::slice::from_raw_parts_mut(b2 as *mut Rgb565Pixel, pixel_count) };

        Some(Self {
            panel,
            buf1,
            buf2,
            use_buf1: true,
        })
    }

    /// Render a frame into back-buffer and swap cleanly with VSYNC synchronization.
    pub fn render_with<F>(&mut self, render_fn: F)
    where
        F: FnOnce(&mut [Rgb565Pixel]),
    {
        let current_buf = if self.use_buf1 {
            &mut *self.buf1
        } else {
            &mut *self.buf2
        };

        // 1. Render into the inactive back buffer
        render_fn(current_buf);

        // 2. Clear VSYNC flag BEFORE submitting the frame so we wait for the NEW VSYNC
        VSYNC.store(false, core::sync::atomic::Ordering::SeqCst);

        // 3. Queue the completed buffer to the LCD DMA controller for next refresh
        unsafe {
            let ret = esp_lcd_panel_draw_bitmap(
                self.panel,
                0,
                0,
                H_RES as i32,
                V_RES as i32,
                current_buf.as_ptr() as *const c_void,
            );
            if ret != ESP_OK {
                log::warn!("esp_lcd_panel_draw_bitmap failed: {}", ret);
            }
        }

        // 4. Wait for the hardware VSYNC event that occurs AFTER draw_bitmap was registered
        let mut timeout_us: u32 = 50_000;
        while !VSYNC.swap(false, core::sync::atomic::Ordering::SeqCst) && timeout_us > 0 {
            esp_idf_hal::task::do_yield();
            esp_idf_hal::delay::Ets::delay_us(100);
            timeout_us = timeout_us.saturating_sub(100);
        }

        // 5. Alternate buffer for next frame: now safe because DMA has switched to current_buf
        self.use_buf1 = !self.use_buf1;
    }

    pub fn dimensions(&self) -> (i32, i32) {
        (H_RES as i32, V_RES as i32)
    }

    pub fn panel(&self) -> esp_lcd_panel_handle_t {
        self.panel
    }
}

impl Drop for LcdDriver {
    fn drop(&mut self) {
        unsafe {
            esp_idf_sys::gpio_set_level(LCD_BACKLIGHT, 0);
            esp_lcd_panel_disp_on_off(self.panel, false);
            esp_lcd_panel_del(self.panel);
        }
    }
}

// ─── Global State ───────────────────────────────────────────────────────────

static G_LCD: LazyLock<Mutex<Option<LcdDriver>>> = LazyLock::new(|| Mutex::new(None));

pub fn init_lcd() -> bool {
    let lcd = LcdDriver::new();
    let mut guard = G_LCD.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(lcd) = lcd {
        *guard = Some(lcd);
        log::info!("LCD platform initialized");
        true
    } else {
        log::error!("Failed to initialize LCD platform");
        false
    }
}

pub fn with_lcd_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut LcdDriver) -> R,
{
    let mut guard = G_LCD.lock().unwrap_or_else(|e| e.into_inner());
    let lcd = match guard.as_mut() {
        Some(l) => l,
        None => {
            log::error!("LCD not initialized. Call init_lcd() first.");
            panic!("LCD not initialized. Call init_lcd() first.");
        }
    };
    f(lcd)
}

pub fn screen_dimensions() -> (i32, i32) {
    (H_RES as i32, V_RES as i32)
}

pub fn set_backlight(on: bool) {
    unsafe {
        esp_idf_sys::gpio_set_level(LCD_BACKLIGHT, if on { 1 } else { 0 });
    }
}



// ─── Slint Platform Implementation ──────────────────────────────────────────

struct SlintWindowHolder(Option<Rc<MinimalSoftwareWindow>>);
unsafe impl Send for SlintWindowHolder {}

static G_SLINT_WINDOW: LazyLock<Mutex<SlintWindowHolder>> =
    LazyLock::new(|| Mutex::new(SlintWindowHolder(None)));

pub fn has_active_window() -> bool {
    let guard = G_SLINT_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
    guard.0.is_some()
}

pub struct EspPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Default for EspPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl EspPlatform {
    pub fn new() -> Self {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::SwappedBuffers);
        window.set_size(PhysicalSize::new(H_RES as u32, V_RES as u32));
        {
            let mut guard = G_SLINT_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
            guard.0 = Some(window.clone());
        }
        Self { window }
    }
}

impl slint::platform::Platform for EspPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        unsafe {
            let us = esp_idf_sys::esp_timer_get_time();
            Duration::from_micros(us as u64)
        }
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(EspEventLoopProxy))
    }
}

struct EspEventLoopProxy;

impl slint::platform::EventLoopProxy for EspEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        event();
        Ok(())
    }
}

pub fn init_slint_platform() -> Result<(), slint::platform::SetPlatformError> {
    slint::platform::set_platform(Box::new(EspPlatform::new()))
}

/// Render the current Slint UI state directly to the LCD with VSYNC swap.
/// Returns `true` if a frame was actually rendered, `false` otherwise.
pub fn flush_to_lcd() -> bool {
    flush_to_lcd_with_overlay(|_, _, _| {})
}

/// Render the current Slint UI state directly to the LCD and apply an overlay before VSYNC swap.
pub fn flush_to_lcd_with_overlay<F: FnOnce(&mut [Rgb565Pixel], usize, usize)>(overlay: F) -> bool {
    let window = {
        let guard = G_SLINT_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        guard.0.clone()
    };
    if let Some(window) = window {
        with_lcd_mut(|lcd| {
            window.draw_if_needed(|renderer| {
                lcd.render_with(|buf| {
                    renderer.render(buf, H_RES);
                    overlay(buf, H_RES, V_RES);
                });
            })
        })
    } else {
        false
    }
}

/// Dispatch a touchscreen pointer event to the active Slint window.
pub fn dispatch_pointer_event(event: slint::platform::WindowEvent) {
    let window = {
        let guard = G_SLINT_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        guard.0.clone()
    };
    if let Some(window) = window {
        window.dispatch_event(event);
    }
}

/// Fully clear both double-buffers to background color (#0c0b0f).
pub fn clear_framebuffers() {
    with_lcd_mut(|lcd| {
        let bg = Rgb565Pixel(0x0841); // #0c0b0f
        for p in lcd.buf1.iter_mut() {
            *p = bg;
        }
        for p in lcd.buf2.iter_mut() {
            *p = bg;
        }
    });
}

