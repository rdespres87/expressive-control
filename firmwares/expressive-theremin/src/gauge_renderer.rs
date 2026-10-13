//! Subpixel antialiased vector renderer for the Strobe Tuner Gauge.
//!
//! Uses distance-based subpixel coverage and RGB565 alpha blending (Wu-style antialiasing)
//! to produce ultra-crisp, perfectly smooth arcs, radial notches, and a pivoting needle.

use slint::platform::software_renderer::Rgb565Pixel;

pub struct StrobeGaugeParams {
    pub cents: f32,       // -50.0 to +50.0
    pub is_active: bool,
}

#[derive(Clone, Copy, Debug)]
struct RgbColor {
    r: u8, // 0..255
    g: u8, // 0..255
    b: u8, // 0..255
}

impl RgbColor {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Fast RGB565 alpha blending directly into the framebuffer.
#[inline(always)]
fn blend_pixel_alpha(
    buf: &mut [Rgb565Pixel],
    stride: usize,
    x: i32,
    y: i32,
    color: RgbColor,
    alpha: f32, // 0.0 to 1.0
) {
    if alpha <= 0.01 {
        return;
    }
    if x < 0 || x >= stride as i32 || !(0..480).contains(&y) {
        return;
    }

    let idx = (y as usize) * stride + (x as usize);
    if idx >= buf.len() {
        return;
    }

    if alpha >= 0.99 {
        // Fast full overwrite
        let p = (((color.r as u16) & 0xF8) << 8)
            | (((color.g as u16) & 0xFC) << 3)
            | ((color.b as u16) >> 3);
        buf[idx] = Rgb565Pixel(p);
        return;
    }

    // Decode existing destination RGB565
    let dst_raw = buf[idx].0;
    let dst_r5 = ((dst_raw >> 11) & 0x1F) as i32;
    let dst_g6 = ((dst_raw >> 5) & 0x3F) as i32;
    let dst_b5 = (dst_raw & 0x1F) as i32;

    // Convert source color to 5/6/5 precision
    let src_r5 = (color.r >> 3) as i32;
    let src_g6 = (color.g >> 2) as i32;
    let src_b5 = (color.b >> 3) as i32;

    // Integer alpha in 0..256
    let a = (alpha * 256.0).clamp(0.0, 256.0) as i32;

    // Blend: out = dst + (src - dst) * a / 256
    let out_r5 = ((dst_r5 + (((src_r5 - dst_r5) * a) >> 8)).clamp(0, 31)) as u16;
    let out_g6 = ((dst_g6 + (((src_g6 - dst_g6) * a) >> 8)).clamp(0, 63)) as u16;
    let out_b5 = ((dst_b5 + (((src_b5 - dst_b5) * a) >> 8)).clamp(0, 31)) as u16;

    buf[idx] = Rgb565Pixel((out_r5 << 11) | (out_g6 << 5) | out_b5);
}

/// Draw a subpixel antialiased line segment with specified thickness.
#[allow(clippy::too_many_arguments)]
fn draw_aa_segment(
    buf: &mut [Rgb565Pixel],
    stride: usize,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    thickness: f32,
    color: RgbColor,
) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.001 {
        return;
    }
    let _len = len_sq.sqrt();
    let half_w = thickness * 0.5;
    let radius = half_w + 1.0;

    // Bounding box of the segment with antialiasing padding
    let min_x = ((x0.min(x1) - radius).floor() as i32).max(0);
    let max_x = ((x0.max(x1) + radius).ceil() as i32).min((stride - 1) as i32);
    let min_y = ((y0.min(y1) - radius).floor() as i32).max(0);
    let max_y = ((y0.max(y1) + radius).ceil() as i32).min(479);

    let inv_len_sq = 1.0 / len_sq;

    for py in min_y..=max_y {
        let fpy = py as f32 + 0.5;
        for px in min_x..=max_x {
            let fpx = px as f32 + 0.5;

            // Project point onto segment: t in [0.0, 1.0]
            let t = (((fpx - x0) * dx + (fpy - y0) * dy) * inv_len_sq).clamp(0.0, 1.0);
            let proj_x = x0 + t * dx;
            let proj_y = y0 + t * dy;

            // Distance from pixel center to closest point on segment
            let dist_sq = (fpx - proj_x) * (fpx - proj_x) + (fpy - proj_y) * (fpy - proj_y);
            let dist = dist_sq.sqrt();

            // Subpixel coverage calculation
            let edge = dist - half_w;
            let alpha = if edge <= -0.5 {
                1.0
            } else if edge < 0.5 {
                0.5 - edge
            } else {
                0.0
            };

            if alpha > 0.01 {
                blend_pixel_alpha(buf, stride, px, py, color, alpha);
            }
        }
    }
}

/// Draw a subpixel antialiased circular arc track.
fn draw_aa_arc(
    buf: &mut [Rgb565Pixel],
    stride: usize,
    cx: f32,
    cy: f32,
    radius: f32,
    thickness: f32,
    color: RgbColor,
) {
    let half_w = thickness * 0.5;
    let r_min = radius - half_w - 1.0;
    let r_max = radius + half_w + 1.0;

    let min_x = ((cx - r_max).floor() as i32).max(0);
    let max_x = ((cx + r_max).ceil() as i32).min((stride - 1) as i32);
    let min_y = ((cy - r_max).floor() as i32).max(0);
    let max_y = (cy.ceil() as i32).min(479); // Semi-circle: top half only

    for py in min_y..=max_y {
        let fpy = py as f32 + 0.5;
        let dy = fpy - cy;
        for px in min_x..=max_x {
            let fpx = px as f32 + 0.5;
            let dx = fpx - cx;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist >= r_min && dist <= r_max {
                let d = (dist - radius).abs();
                let edge = d - half_w;
                let alpha = if edge <= -0.5 {
                    1.0
                } else if edge < 0.5 {
                    0.5 - edge
                } else {
                    0.0
                };

                if alpha > 0.01 {
                    blend_pixel_alpha(buf, stride, px, py, color, alpha);
                }
            }
        }
    }
}

/// Draw the Strobe Tuner Gauge with pristine subpixel antialiasing.
pub fn draw_strobe_gauge(
    buf: &mut [Rgb565Pixel],
    stride: usize,
    _height: usize,
    params: &StrobeGaugeParams,
) {
    let cx: f32 = 400.0;
    let cy: f32 = 265.0; // Lowered to leave a clean 33px top margin
    let r_track: f32 = 232.0;
    let pi = std::f32::consts::PI;

    // 0. Clear gauge area with background color (#0c0b0f) to prevent ghosting between double buffers.
    // At y < 85, the circular gauge is centered around x=400 and does not exceed x=550.
    // Restricting x_end to 560 for y < 85 guarantees the top-right Config button (x: 610..780, y: 15..71)
    // is never clipped or overwritten by the gauge renderer.
    let bg_pixel = Rgb565Pixel(0x0841);
    for y in 14..=278 {
        let row_start = y * stride;
        let x_start = if y < 85 { 240 } else { 140 };
        let x_end = if y < 85 { 560 } else { 650 };
        for x in x_start..=x_end {
            buf[row_start + x] = bg_pixel;
        }
    }

    // Palette
    let col_track = RgbColor::new(42, 42, 60);         // Smooth slate circular rail
    let col_tick_idle = RgbColor::new(60, 60, 84);     // Crisp unlit ticks
    let col_tick_major = RgbColor::new(160, 160, 190); // Calibration ticks
    let col_center_ref = RgbColor::new(255, 255, 255); // White center notch

    let col_green = RgbColor::new(0, 255, 136);        // Neon green (<= 10 cents)
    let col_orange = RgbColor::new(255, 170, 0);       // Warm orange (<= 20 cents)
    let col_red = RgbColor::new(255, 51, 68);          // Crimson red (> 20 cents)

    let clamped_cents = params.cents.clamp(-50.0, 50.0);
    let abs_cents = clamped_cents.abs();
    let tune_color = if !params.is_active {
        RgbColor::new(70, 70, 95)
    } else if abs_cents <= 10.0 {
        col_green
    } else if abs_cents <= 20.0 {
        col_orange
    } else {
        col_red
    };

    // 1. Antialiased continuous arc track (R = 232px, bold thickness 20.0px)
    draw_aa_arc(buf, stride, cx, cy, r_track, 20.0, col_track);

    // 2. 101 Radial Strobe Notches (Heavily thickened for high impact)
    for i in 0..=100 {
        let tick_cents = -50.0 + (i as f32) * 1.0;
        let a = -pi / 2.0 + (i as f32 / 100.0) * pi;

        let sin_a = a.sin();
        let cos_a = a.cos();

        // Thickened tick profiles:
        // Regular ticks: thickness 5.0px, inner radius 212px, outer radius 242px
        // Major 10-cent ticks: thickness 7.5px, inner radius 200px, outer radius 242px
        // Center tick: thickness 10.0px, inner radius 188px, outer radius 244px
        let (r_in, r_out, thickness) = if i == 50 {
            (188.0, 244.0, 10.0)
        } else if i % 10 == 0 {
            (200.0, 242.0, 7.5)
        } else {
            (212.0, 242.0, 5.0)
        };

        let x0 = cx + r_in * sin_a;
        let y0 = cy - r_in * cos_a;
        let x1 = cx + r_out * sin_a;
        let y1 = cy - r_out * cos_a;

        let is_lit = params.is_active && (
            (clamped_cents <= -0.5 && tick_cents >= clamped_cents && tick_cents <= 0.0) ||
            (clamped_cents >= 0.5 && tick_cents <= clamped_cents && tick_cents >= 0.0) ||
            (abs_cents < 0.5 && i == 50)
        );

        let color = if is_lit {
            tune_color
        } else if i == 50 {
            col_center_ref
        } else if i % 10 == 0 {
            col_tick_major
        } else {
            col_tick_idle
        };

        draw_aa_segment(buf, stride, x0, y0, x1, y1, thickness, color);
    }

    // 3. Pivoting Needle (Heavy instrument-style 10.0px needle with 18.0px diamond head)
    if params.is_active {
        // Strictly constrained to [-pi/2, +pi/2] (upper semi-circle only)
        let needle_a = (clamped_cents / 50.0) * (pi / 2.0);
        let sin_n = needle_a.sin();
        let cos_n = needle_a.cos().max(0.0); // Never point below the horizontal line

        // Needle shaft from R = 90px to R = 236px, thickness 10.0px
        let n_x0 = cx + 90.0 * sin_n;
        let n_y0 = cy - 90.0 * cos_n;
        let n_x1 = cx + 236.0 * sin_n;
        let n_y1 = cy - 236.0 * cos_n;

        // Needle core (tune color, thickness 10.0px)
        draw_aa_segment(buf, stride, n_x0, n_y0, n_x1, n_y1, 10.0, tune_color);

        // Center hub dot (R = 90px, diameter 16.0px)
        let hub_color = col_center_ref;
        draw_aa_segment(buf, stride, n_x0, n_y0, n_x0, n_y0, 16.0, hub_color);

        // Profiled Needle Tip (Antialiased pointer at R = 236px, size 18.0px)
        let tip_x = cx + 236.0 * sin_n;
        let tip_y = cy - 236.0 * cos_n;
        draw_aa_segment(buf, stride, tip_x, tip_y, tip_x, tip_y, 18.0, col_center_ref);
    }
}
