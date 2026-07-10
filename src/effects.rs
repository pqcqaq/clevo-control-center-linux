use std::time::Duration;

use crate::model::{Mode, Rgb, ZoneColor};
use crate::settings::Settings;

pub fn colors_for_mode(mode: Mode, phase: f32, settings: &Settings) -> Vec<ZoneColor> {
    let brightness = settings.brightness as f32 / 100.0;
    let zones = settings.zones.clone();

    match mode {
        Mode::Custom => zones
            .into_iter()
            .map(|zone| ZoneColor {
                zone,
                rgb: settings.f0_color,
            })
            .collect(),
        Mode::Cycle => {
            let rgb = hsv_rgb(phase, 1.0, brightness);
            zones
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
        Mode::Chase => {
            let zone_count = zones.len().max(1) as f32;
            zones
                .into_iter()
                .enumerate()
                .map(|(index, zone)| ZoneColor {
                    zone,
                    rgb: hsv_rgb(phase + index as f32 / zone_count, 1.0, brightness),
                })
                .collect()
        }
        Mode::Blink => {
            let blink_phase = (phase * 5.0).fract();
            let level = if blink_phase < 0.42 {
                1.0
            } else if blink_phase < 0.5 {
                1.0 - smoothstep((blink_phase - 0.42) / 0.08)
            } else if blink_phase < 0.92 {
                0.0
            } else {
                smoothstep((blink_phase - 0.92) / 0.08)
            };
            let rgb = scale_rgb(settings.f0_color, brightness * level);
            zones
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
        Mode::Breathing => {
            let pulse = 0.12 + 0.88 * ((phase * std::f32::consts::TAU).sin() + 1.0) / 2.0;
            let rgb = scale_rgb(settings.f0_color, pulse * brightness);
            zones
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
    }
}

pub fn tick_interval(speed: u8) -> Duration {
    let speed = speed.clamp(1, 100) as u64;
    let millis = 130_u64.saturating_sub(((speed - 1) * 40) / 99);
    Duration::from_millis(millis)
}

pub fn cycles_per_second(speed: u8) -> f32 {
    let t = speed.clamp(1, 100) as f32 / 100.0;
    0.035 + 0.42 * t.powf(1.35)
}

pub fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn scale_rgb(rgb: Rgb, factor: f32) -> Rgb {
    Rgb {
        r: clamp_u8(rgb.r as f32 * factor),
        g: clamp_u8(rgb.g as f32 * factor),
        b: clamp_u8(rgb.b as f32 * factor),
    }
}

fn clamp_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub fn hsv_rgb(hue: f32, saturation: f32, value: f32) -> Rgb {
    let h = hue.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - f * saturation);
    let t = value * (1.0 - (1.0 - f) * saturation);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    Rgb {
        r: clamp_u8(r * 255.0),
        g: clamp_u8(g * 255.0),
        b: clamp_u8(b * 255.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{default_zones, ZoneId};

    #[test]
    fn hsv_cycle_starts_at_red() {
        assert_eq!(hsv_rgb(0.0, 1.0, 1.0), Rgb { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn effect_timing_limits_dchu_update_rate() {
        assert_eq!(tick_interval(1), Duration::from_millis(130));
        assert_eq!(tick_interval(100), Duration::from_millis(90));
        assert!(cycles_per_second(100) < 0.5);
    }

    #[test]
    fn smoothstep_has_stable_edges() {
        assert_eq!(smoothstep(-1.0), 0.0);
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(2.0), 1.0);
    }

    #[test]
    fn service_generates_chase_colors_for_base_zones() {
        let settings = Settings {
            mode: Mode::Chase,
            speed: 50,
            brightness: 100,
            running: true,
            f0_color: Rgb::WHITE,
            zones: default_zones(),
            window_pos: None,
        };

        let colors = colors_for_mode(Mode::Chase, 0.0, &settings);

        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0].zone, ZoneId::F0);
        assert_eq!(colors[1].zone, ZoneId::F1);
        assert_eq!(colors[2].zone, ZoneId::F2);
    }

    #[test]
    fn service_uses_selected_zones() {
        let settings = Settings {
            mode: Mode::Cycle,
            speed: 50,
            brightness: 100,
            running: true,
            f0_color: Rgb::WHITE,
            zones: vec![ZoneId::F0, ZoneId::F4, ZoneId::F6],
            window_pos: None,
        };

        let zones = colors_for_mode(Mode::Cycle, 0.0, &settings)
            .into_iter()
            .map(|color| color.zone)
            .collect::<Vec<_>>();

        assert_eq!(zones, vec![ZoneId::F0, ZoneId::F4, ZoneId::F6]);
    }
}
