//! Expressive "Over the Rainbow" synthetic simulation generator for the theremin.
//!
//! Generates continuous, lifelike theremin performance trajectories:
//! - Smooth cubic hermite portamento (glissando) between notes.
//! - Natural delayed vibrato (~5.3 Hz LFO with gentle attack).
//! - Expressive volume envelope (breathing, crescendos, and phrasing).
//! - Direct translation into ToF distance coordinates (pitch_z, vol_z in mm).

#[derive(Clone, Copy)]
pub struct RainbowNote {
    pub pitch: f32,          // Target MIDI note (e.g. 65.0 for F4, 77.0 for F5)
    pub duration_s: f32,     // Note duration in seconds
    pub target_vol: f32,     // Target volume (0.0 to 1.0)
    pub glide_s: f32,        // Portamento duration from previous pitch
    pub vib_delay_s: f32,    // Seconds before vibrato starts
    pub vib_depth_cents: f32,// Vibrato amplitude in cents (e.g. 35.0)
    pub breath_after_s: f32, // Volume dip at the end of the note for phrasing
}

pub const RAINBOW_SCORE: &[RainbowNote] = &[
    // --- Phrase 1: "Somewhere over the rainbow..." ---
    // "Some-" (F4 = 65.0) - Gentle entrance, swelling
    RainbowNote {
        pitch: 65.0,
        duration_s: 2.2,
        target_vol: 0.75,
        glide_s: 0.0,
        vib_delay_s: 0.8,
        vib_depth_cents: 25.0,
        breath_after_s: 0.0, // Legato into the octave jump
    },
    // "-where" (F5 = 77.0) - The iconic soaring octave glissando!
    RainbowNote {
        pitch: 77.0,
        duration_s: 2.6,
        target_vol: 0.95,
        glide_s: 0.45,       // 450ms continuous glissando slide
        vib_delay_s: 0.35,
        vib_depth_cents: 38.0,
        breath_after_s: 0.15,
    },
    // "o-" (E5 = 76.0)
    RainbowNote {
        pitch: 76.0,
        duration_s: 1.1,
        target_vol: 0.82,
        glide_s: 0.18,
        vib_delay_s: 0.25,
        vib_depth_cents: 30.0,
        breath_after_s: 0.0,
    },
    // "-ver" (C5 = 72.0)
    RainbowNote {
        pitch: 72.0,
        duration_s: 1.0,
        target_vol: 0.78,
        glide_s: 0.22,
        vib_delay_s: 0.20,
        vib_depth_cents: 28.0,
        breath_after_s: 0.0,
    },
    // "the" (D5 = 74.0)
    RainbowNote {
        pitch: 74.0,
        duration_s: 0.6,
        target_vol: 0.80,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 22.0,
        breath_after_s: 0.0,
    },
    // "rain-" (E5 = 76.0)
    RainbowNote {
        pitch: 76.0,
        duration_s: 0.6,
        target_vol: 0.85,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 25.0,
        breath_after_s: 0.0,
    },
    // "-bow" (F5 = 77.0) - Climax of line 1
    RainbowNote {
        pitch: 77.0,
        duration_s: 2.8,
        target_vol: 0.95,
        glide_s: 0.25,
        vib_delay_s: 0.30,
        vib_depth_cents: 40.0,
        breath_after_s: 0.30, // Left hand dips for a breath
    },

    // --- Phrase 2: "Way up high..." ---
    // "way" (F5 = 77.0)
    RainbowNote {
        pitch: 77.0,
        duration_s: 1.4,
        target_vol: 0.82,
        glide_s: 0.0,
        vib_delay_s: 0.25,
        vib_depth_cents: 32.0,
        breath_after_s: 0.0,
    },
    // "up" (D5 = 74.0)
    RainbowNote {
        pitch: 74.0,
        duration_s: 1.2,
        target_vol: 0.78,
        glide_s: 0.22,
        vib_delay_s: 0.20,
        vib_depth_cents: 30.0,
        breath_after_s: 0.0,
    },
    // "high" (C5 = 72.0)
    RainbowNote {
        pitch: 72.0,
        duration_s: 2.8,
        target_vol: 0.85,
        glide_s: 0.25,
        vib_delay_s: 0.30,
        vib_depth_cents: 35.0,
        breath_after_s: 0.50, // Decrescendo into silence
    },

    // --- Phrase 3: "There's a land that I heard of..." ---
    // "There's" (F4 = 65.0)
    RainbowNote {
        pitch: 65.0,
        duration_s: 1.0,
        target_vol: 0.70,
        glide_s: 0.0,
        vib_delay_s: 0.25,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "a" (D5 = 74.0) - Expressive major 6th swoop upward!
    RainbowNote {
        pitch: 74.0,
        duration_s: 1.1,
        target_vol: 0.85,
        glide_s: 0.35,
        vib_delay_s: 0.25,
        vib_depth_cents: 32.0,
        breath_after_s: 0.0,
    },
    // "land" (Bb4 = 70.0)
    RainbowNote {
        pitch: 70.0,
        duration_s: 1.8,
        target_vol: 0.82,
        glide_s: 0.25,
        vib_delay_s: 0.30,
        vib_depth_cents: 35.0,
        breath_after_s: 0.0,
    },
    // "that" (A4 = 69.0)
    RainbowNote {
        pitch: 69.0,
        duration_s: 0.7,
        target_vol: 0.75,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 22.0,
        breath_after_s: 0.0,
    },
    // "I" (F4 = 65.0)
    RainbowNote {
        pitch: 65.0,
        duration_s: 0.7,
        target_vol: 0.72,
        glide_s: 0.18,
        vib_delay_s: 0.15,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "heard" (G4 = 67.0)
    RainbowNote {
        pitch: 67.0,
        duration_s: 0.7,
        target_vol: 0.75,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "of" (A4 = 69.0)
    RainbowNote {
        pitch: 69.0,
        duration_s: 0.7,
        target_vol: 0.78,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "once" (Bb4 = 70.0)
    RainbowNote {
        pitch: 70.0,
        duration_s: 1.2,
        target_vol: 0.82,
        glide_s: 0.18,
        vib_delay_s: 0.20,
        vib_depth_cents: 28.0,
        breath_after_s: 0.0,
    },
    // "in" (G4 = 67.0)
    RainbowNote {
        pitch: 67.0,
        duration_s: 0.8,
        target_vol: 0.72,
        glide_s: 0.18,
        vib_delay_s: 0.15,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "a" (E4 = 64.0)
    RainbowNote {
        pitch: 64.0,
        duration_s: 0.8,
        target_vol: 0.70,
        glide_s: 0.18,
        vib_delay_s: 0.15,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "lul-" (F4 = 65.0)
    RainbowNote {
        pitch: 65.0,
        duration_s: 0.8,
        target_vol: 0.72,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 20.0,
        breath_after_s: 0.0,
    },
    // "-la-" (G4 = 67.0)
    RainbowNote {
        pitch: 67.0,
        duration_s: 0.8,
        target_vol: 0.76,
        glide_s: 0.15,
        vib_delay_s: 0.15,
        vib_depth_cents: 22.0,
        breath_after_s: 0.0,
    },
    // "-by..." (F4 = 65.0) - Grand resolution with tender fade-out
    RainbowNote {
        pitch: 65.0,
        duration_s: 4.2,
        target_vol: 0.90,
        glide_s: 0.25,
        vib_delay_s: 0.35,
        vib_depth_cents: 38.0,
        breath_after_s: 2.2, // 2.2s emotional diminuendo into total silence
    },

    // --- Rest / Breath before looping ---
    RainbowNote {
        pitch: 65.0,
        duration_s: 1.5,
        target_vol: 0.0,
        glide_s: 0.0,
        vib_delay_s: 0.0,
        vib_depth_cents: 0.0,
        breath_after_s: 0.0,
    },
];

/// Calculates the total duration of the score in seconds.
pub fn total_score_duration() -> f32 {
    let mut total = 0.0f32;
    let mut i = 0;
    while i < RAINBOW_SCORE.len() {
        total += RAINBOW_SCORE[i].duration_s;
        i += 1;
    }
    total
}

/// Computes the synthetic (pitch_z, vol_z) distance measurements at a given step (100 Hz).
/// Returns `(pitch_z_mm, vol_z_mm)`.
pub fn compute_rainbow_simulation(step: u32) -> (u16, u16) {
    let loop_duration = total_score_duration();
    let current_time_total = (step as f32) * 0.01; // 100 Hz = 0.01s per step
    let time_in_loop = current_time_total % loop_duration;

    // 1. Locate current note in the score
    let mut elapsed = 0.0f32;
    let mut current_idx = 0;
    let mut note_start_time = 0.0f32;

    for (i, note) in RAINBOW_SCORE.iter().enumerate() {
        if time_in_loop < elapsed + note.duration_s {
            current_idx = i;
            note_start_time = elapsed;
            break;
        }
        elapsed += note.duration_s;
    }

    let current_note = &RAINBOW_SCORE[current_idx];
    let prev_pitch = if current_idx == 0 {
        RAINBOW_SCORE[RAINBOW_SCORE.len() - 1].pitch
    } else {
        RAINBOW_SCORE[current_idx - 1].pitch
    };

    let t_in_note = time_in_loop - note_start_time;
    let remaining_in_note = current_note.duration_s - t_in_note;

    // 2. Pitch computation: Portamento (Glissando)
    let base_pitch = if current_note.glide_s > 0.001 && t_in_note < current_note.glide_s {
        let u = (t_in_note / current_note.glide_s).clamp(0.0, 1.0);
        let s = u * u * (3.0 - 2.0 * u); // Smooth cubic hermite curve
        prev_pitch + (current_note.pitch - prev_pitch) * s
    } else {
        current_note.pitch
    };

    // 3. Pitch computation: Delayed human vibrato (~5.3 Hz)
    let pitch = if t_in_note > current_note.vib_delay_s && current_note.vib_depth_cents > 0.1 {
        let vib_time = t_in_note - current_note.vib_delay_s;
        let vib_blend = (vib_time / 0.45).clamp(0.0, 1.0); // 450ms fade-in of vibrato
        let lfo = (vib_time * 5.3 * 2.0 * core::f32::consts::PI).sin();
        let lfo_asym = lfo + 0.08 * (vib_time * 10.6 * 2.0 * core::f32::consts::PI).sin();
        let vib_semis = lfo_asym * (current_note.vib_depth_cents / 100.0) * vib_blend;
        base_pitch + vib_semis
    } else {
        base_pitch
    };

    // 4. Volume computation: Attack, Phrasing, Breath, and Diminuendo
    let vol = if current_note.target_vol <= 0.001 {
        0.0f32
    } else {
        let attack_time = 0.12f32;
        let attack_progress = (t_in_note / attack_time).clamp(0.0, 1.0);
        let mut v = current_note.target_vol * attack_progress;

        // Phrasing dip / breath at note end
        if current_note.breath_after_s > 0.001 && remaining_in_note < current_note.breath_after_s {
            let breath_factor = (remaining_in_note / current_note.breath_after_s).clamp(0.0, 1.0);
            v *= breath_factor;
        }

        // Natural subtle volume tremolo synchronized with vibrato (~4%)
        if t_in_note > current_note.vib_delay_s && current_note.vib_depth_cents > 0.1 {
            let vib_time = t_in_note - current_note.vib_delay_s;
            let tremolo = 1.0 + 0.04 * (vib_time * 5.3 * 2.0 * core::f32::consts::PI).sin();
            v = (v * tremolo).clamp(0.0, 1.0);
        }
        v
    };

    // 5. Map pitch and volume to ToF sensor millimeters (z-axis)
    // C4 (60.0) to C6 (84.0) over 1000mm to 20mm
    let semitones_from_c4 = (pitch - 60.0).clamp(0.0, 24.0);
    let pitch_mm = 1000.0 - semitones_from_c4 * (980.0 / 24.0);
    let pitch_z = pitch_mm.clamp(20.0, 1000.0) as u16;

    // Volume: 0.0 (silent, 1000mm) to 1.0 (loud, 20mm)
    let vol_mm = 1000.0 - vol.clamp(0.0, 1.0) * 980.0;
    let vol_z = vol_mm.clamp(20.0, 1000.0) as u16;

    (pitch_z, vol_z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_duration_bounds() {
        let dur = total_score_duration();
        assert!(dur >= 25.0 && dur <= 45.0, "Score duration {}s is out of expected bounds", dur);
    }

    #[test]
    fn test_simulation_range_and_continuity() {
        let total_steps = (total_score_duration() * 100.0) as u32;
        for step in (0..total_steps).step_by(5) {
            let (pitch_z, vol_z) = compute_rainbow_simulation(step);
            assert!(
                pitch_z >= 20 && pitch_z <= 1000,
                "pitch_z {} out of range at step {}",
                pitch_z,
                step
            );
            assert!(
                vol_z >= 20 && vol_z <= 1000,
                "vol_z {} out of range at step {}",
                vol_z,
                step
            );
        }
    }

    #[test]
    fn test_octave_glissando_smoothness() {
        // At step 220 (2.2s), "Some-" (F4) ends and "-where" (F5) starts gliding
        let (p_start, _) = compute_rainbow_simulation(220); // Note F4 (z ~= 796)
        let (p_end, _) = compute_rainbow_simulation(270);   // Note F5 (z ~= 306)
        assert!(p_start > p_end, "Glissando must move upwards (smaller distance)");

        // Verify smoothness: step-by-step delta should never exceed 25mm per 10ms
        let mut prev_p = p_start;
        for step in 221..=270 {
            let (p, _) = compute_rainbow_simulation(step);
            let delta = (p as i32 - prev_p as i32).abs();
            assert!(delta <= 25, "Portamento jump {}mm too large at step {}", delta, step);
            prev_p = p;
        }
    }
}
