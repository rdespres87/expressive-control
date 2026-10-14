//! Subpixel antialiased vector renderer for the Strobe Tuner Gauge.
//!
//! Uses distance-based subpixel coverage and RGB565 alpha blending (Wu-style antialiasing)
//! to produce ultra-crisp, perfectly smooth arcs, radial notches, and a pivoting needle.

use slint::platform::software_renderer::Rgb565Pixel;

pub struct StrobeGaugeParams {
    pub cents: f32, // -50.0 to +50.0
    pub is_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8, // 0..255
    pub g: u8, // 0..255
    pub b: u8, // 0..255
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// 2D point representation for vector drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A line segment with subpixel thickness and color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSegment {
    pub start: Point2D,
    pub end: Point2D,
    pub thickness: f32,
    pub color: RgbColor,
}

impl LineSegment {
    pub const fn new(start: Point2D, end: Point2D, thickness: f32, color: RgbColor) -> Self {
        Self {
            start,
            end,
            thickness,
            color,
        }
    }
}

/// A circular arc track segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircularArc {
    pub center: Point2D,
    pub radius: f32,
    pub thickness: f32,
    pub color: RgbColor,
}

impl CircularArc {
    pub const fn new(center: Point2D, radius: f32, thickness: f32, color: RgbColor) -> Self {
        Self {
            center,
            radius,
            thickness,
            color,
        }
    }
}

/// A filled antialiased circular dot (e.g. needle hub, needle tip).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilledCircle {
    pub center: Point2D,
    pub radius: f32,
    pub color: RgbColor,
}

impl FilledCircle {
    pub const fn new(center: Point2D, radius: f32, color: RgbColor) -> Self {
        Self {
            center,
            radius,
            color,
        }
    }
}

/// Classification of a radial strobe tick on the tuner scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickKind {
    /// Reference 0-cent center tick.
    Center,
    /// 10-cent calibration intervals.
    Major,
    /// Regular 1-cent ticks.
    Minor,
}

impl TickKind {
    pub const fn from_index(index: usize) -> Self {
        if index == 50 {
            TickKind::Center
        } else if index % 10 == 0 {
            TickKind::Major
        } else {
            TickKind::Minor
        }
    }

    /// Returns `(inner_radius, outer_radius, thickness)` for radial tick geometry.
    pub const fn dimensions(self) -> (f32, f32, f32) {
        match self {
            TickKind::Center => (188.0, 244.0, 10.0),
            TickKind::Major => (200.0, 242.0, 7.5),
            TickKind::Minor => (212.0, 242.0, 5.0),
        }
    }

    pub fn default_color(self, center_ref: RgbColor, major: RgbColor, idle: RgbColor) -> RgbColor {
        match self {
            TickKind::Center => center_ref,
            TickKind::Major => major,
            TickKind::Minor => idle,
        }
    }
}

/// Musical tuning pitch accuracy state and associated visual theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuningStatus {
    Inactive,
    InTune,    // <= 10 cents
    NearTune,  // <= 20 cents
    OutOfTune, // > 20 cents
}

impl TuningStatus {
    pub fn evaluate(cents: f32, is_active: bool) -> Self {
        if !is_active {
            TuningStatus::Inactive
        } else {
            let abs_cents = cents.abs();
            if abs_cents <= 10.0 {
                TuningStatus::InTune
            } else if abs_cents <= 20.0 {
                TuningStatus::NearTune
            } else {
                TuningStatus::OutOfTune
            }
        }
    }

    pub fn color(self) -> RgbColor {
        match self {
            TuningStatus::Inactive => RgbColor::new(70, 70, 95),
            TuningStatus::InTune => RgbColor::new(0, 255, 136), // Neon green
            TuningStatus::NearTune => RgbColor::new(255, 170, 0), // Warm orange
            TuningStatus::OutOfTune => RgbColor::new(255, 51, 68), // Crimson red
        }
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
fn draw_aa_segment(buf: &mut [Rgb565Pixel], stride: usize, segment: LineSegment) {
    let dx = segment.end.x - segment.start.x;
    let dy = segment.end.y - segment.start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.001 {
        return;
    }
    let half_w = segment.thickness * 0.5;
    let radius = half_w + 1.0;

    // Bounding box of the segment with antialiasing padding
    let min_x = ((segment.start.x.min(segment.end.x) - radius).floor() as i32).max(0);
    let max_x =
        ((segment.start.x.max(segment.end.x) + radius).ceil() as i32).min((stride - 1) as i32);
    let min_y = ((segment.start.y.min(segment.end.y) - radius).floor() as i32).max(0);
    let max_y = ((segment.start.y.max(segment.end.y) + radius).ceil() as i32).min(479);

    let inv_len_sq = 1.0 / len_sq;

    for py in min_y..=max_y {
        let fpy = py as f32 + 0.5;
        for px in min_x..=max_x {
            let fpx = px as f32 + 0.5;

            // Project point onto segment: t in [0.0, 1.0]
            let t = (((fpx - segment.start.x) * dx + (fpy - segment.start.y) * dy) * inv_len_sq)
                .clamp(0.0, 1.0);
            let proj_x = segment.start.x + t * dx;
            let proj_y = segment.start.y + t * dy;

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
                blend_pixel_alpha(buf, stride, px, py, segment.color, alpha);
            }
        }
    }
}

/// Draw a filled subpixel antialiased circle.
fn draw_aa_circle(buf: &mut [Rgb565Pixel], stride: usize, circle: FilledCircle) {
    let r_max = circle.radius + 1.0;
    let min_x = ((circle.center.x - r_max).floor() as i32).max(0);
    let max_x = ((circle.center.x + r_max).ceil() as i32).min((stride - 1) as i32);
    let min_y = ((circle.center.y - r_max).floor() as i32).max(0);
    let max_y = ((circle.center.y + r_max).ceil() as i32).min(479);

    for py in min_y..=max_y {
        let fpy = py as f32 + 0.5;
        let dy = fpy - circle.center.y;
        for px in min_x..=max_x {
            let fpx = px as f32 + 0.5;
            let dx = fpx - circle.center.x;
            let dist = (dx * dx + dy * dy).sqrt();

            let edge = dist - circle.radius;
            let alpha = if edge <= -0.5 {
                1.0
            } else if edge < 0.5 {
                0.5 - edge
            } else {
                0.0
            };

            if alpha > 0.01 {
                blend_pixel_alpha(buf, stride, px, py, circle.color, alpha);
            }
        }
    }
}

/// Draw a subpixel antialiased circular arc track.
fn draw_aa_arc(buf: &mut [Rgb565Pixel], stride: usize, arc: CircularArc) {
    let half_w = arc.thickness * 0.5;
    let r_min = arc.radius - half_w - 1.0;
    let r_max = arc.radius + half_w + 1.0;

    let min_x = ((arc.center.x - r_max).floor() as i32).max(0);
    let max_x = ((arc.center.x + r_max).ceil() as i32).min((stride - 1) as i32);
    let min_y = ((arc.center.y - r_max).floor() as i32).max(0);
    let max_y = (arc.center.y.ceil() as i32).min(479); // Semi-circle: top half only

    for py in min_y..=max_y {
        let fpy = py as f32 + 0.5;
        let dy = fpy - arc.center.y;
        for px in min_x..=max_x {
            let fpx = px as f32 + 0.5;
            let dx = fpx - arc.center.x;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist >= r_min && dist <= r_max {
                let d = (dist - arc.radius).abs();
                let edge = d - half_w;
                let alpha = if edge <= -0.5 {
                    1.0
                } else if edge < 0.5 {
                    0.5 - edge
                } else {
                    0.0
                };

                if alpha > 0.01 {
                    blend_pixel_alpha(buf, stride, px, py, arc.color, alpha);
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
    let center = Point2D::new(400.0, 265.0); // Lowered to leave a clean 33px top margin
    let r_track: f32 = 232.0;
    let pi = core::f32::consts::PI;

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
    let col_track = RgbColor::new(42, 42, 60); // Smooth slate circular rail
    let col_tick_idle = RgbColor::new(60, 60, 84); // Crisp unlit ticks
    let col_tick_major = RgbColor::new(160, 160, 190); // Calibration ticks
    let col_center_ref = RgbColor::new(255, 255, 255); // White center notch

    let clamped_cents = params.cents.clamp(-50.0, 50.0);
    let abs_cents = clamped_cents.abs();
    let tuning_status = TuningStatus::evaluate(clamped_cents, params.is_active);

    // 1. Antialiased continuous arc track (R = 232px, bold thickness 20.0px)
    draw_aa_arc(
        buf,
        stride,
        CircularArc::new(center, r_track, 20.0, col_track),
    );

    // 2. 101 Radial Strobe Notches (Heavily thickened for high impact)
    for i in 0..=100 {
        let tick_cents = -50.0 + (i as f32) * 1.0;
        let a = -pi / 2.0 + (i as f32 / 100.0) * pi;

        let sin_a = a.sin();
        let cos_a = a.cos();

        let kind = TickKind::from_index(i);
        let (r_in, r_out, thickness) = kind.dimensions();

        let start = Point2D::new(center.x + r_in * sin_a, center.y - r_in * cos_a);
        let end = Point2D::new(center.x + r_out * sin_a, center.y - r_out * cos_a);

        let is_lit = params.is_active
            && ((clamped_cents <= -0.5 && tick_cents >= clamped_cents && tick_cents <= 0.0)
                || (clamped_cents >= 0.5 && tick_cents <= clamped_cents && tick_cents >= 0.0)
                || (abs_cents < 0.5 && i == 50));

        let color = if is_lit {
            tuning_status.color()
        } else {
            kind.default_color(col_center_ref, col_tick_major, col_tick_idle)
        };

        draw_aa_segment(buf, stride, LineSegment::new(start, end, thickness, color));
    }

    // 3. Pivoting Needle (Heavy instrument-style 10.0px needle with 18.0px diamond head)
    if params.is_active {
        // Strictly constrained to [-pi/2, +pi/2] (upper semi-circle only)
        let needle_a = (clamped_cents / 50.0) * (pi / 2.0);
        let sin_n = needle_a.sin();
        let cos_n = needle_a.cos().max(0.0); // Never point below the horizontal line

        // Needle shaft from R = 90px to R = 236px, thickness 10.0px
        let n_start = Point2D::new(center.x + 90.0 * sin_n, center.y - 90.0 * cos_n);
        let n_end = Point2D::new(center.x + 236.0 * sin_n, center.y - 236.0 * cos_n);

        // Needle core (tune color, thickness 10.0px)
        draw_aa_segment(
            buf,
            stride,
            LineSegment::new(n_start, n_end, 10.0, tuning_status.color()),
        );

        // Center hub dot (R = 90px, radius 8.0px -> diameter 16.0px)
        draw_aa_circle(buf, stride, FilledCircle::new(n_start, 8.0, col_center_ref));

        // Profiled Needle Tip (pointer at R = 236px, radius 9.0px -> diameter 18.0px)
        draw_aa_circle(buf, stride, FilledCircle::new(n_end, 9.0, col_center_ref));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_kind_classification() {
        assert_eq!(TickKind::from_index(50), TickKind::Center);
        assert_eq!(TickKind::from_index(0), TickKind::Major);
        assert_eq!(TickKind::from_index(10), TickKind::Major);
        assert_eq!(TickKind::from_index(20), TickKind::Major);
        assert_eq!(TickKind::from_index(1), TickKind::Minor);
        assert_eq!(TickKind::from_index(49), TickKind::Minor);
    }

    #[test]
    fn test_tick_kind_dimensions() {
        let (r_in_c, r_out_c, th_c) = TickKind::Center.dimensions();
        assert_eq!(r_in_c, 188.0);
        assert_eq!(r_out_c, 244.0);
        assert_eq!(th_c, 10.0);

        let (r_in_m, r_out_m, th_m) = TickKind::Major.dimensions();
        assert_eq!(r_in_m, 200.0);
        assert_eq!(r_out_m, 242.0);
        assert_eq!(th_m, 7.5);

        let (r_in_min, r_out_min, th_min) = TickKind::Minor.dimensions();
        assert_eq!(r_in_min, 212.0);
        assert_eq!(r_out_min, 242.0);
        assert_eq!(th_min, 5.0);
    }

    #[test]
    fn test_tuning_status_evaluation() {
        // Inactive always yields Inactive status
        assert_eq!(TuningStatus::evaluate(0.0, false), TuningStatus::Inactive);
        assert_eq!(TuningStatus::evaluate(15.0, false), TuningStatus::Inactive);
        assert_eq!(TuningStatus::evaluate(30.0, false), TuningStatus::Inactive);

        // Active state evaluation
        assert_eq!(TuningStatus::evaluate(0.0, true), TuningStatus::InTune);
        assert_eq!(TuningStatus::evaluate(5.0, true), TuningStatus::InTune);
        assert_eq!(TuningStatus::evaluate(-10.0, true), TuningStatus::InTune);

        assert_eq!(TuningStatus::evaluate(10.1, true), TuningStatus::NearTune);
        assert_eq!(TuningStatus::evaluate(-20.0, true), TuningStatus::NearTune);

        assert_eq!(TuningStatus::evaluate(20.1, true), TuningStatus::OutOfTune);
        assert_eq!(TuningStatus::evaluate(-45.0, true), TuningStatus::OutOfTune);
    }

    #[test]
    fn test_point_and_segment_creation() {
        let p1 = Point2D::new(10.0, 20.0);
        let p2 = Point2D::new(30.0, 40.0);
        let color = RgbColor::new(255, 0, 0);
        let seg = LineSegment::new(p1, p2, 5.0, color);

        assert_eq!(seg.start, p1);
        assert_eq!(seg.end, p2);
        assert_eq!(seg.thickness, 5.0);
        assert_eq!(seg.color, color);
    }

    #[test]
    fn test_circle_creation() {
        let p = Point2D::new(100.0, 150.0);
        let col = RgbColor::new(0, 255, 0);
        let circle = FilledCircle::new(p, 8.0, col);

        assert_eq!(circle.center, p);
        assert_eq!(circle.radius, 8.0);
        assert_eq!(circle.color, col);
    }
}
