use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use crate::dchu::{KeyboardLightingCapabilities, KeyboardLightingLayout};
use crate::model::{LightingConfig, LightingFrame, Mode, Rgb, ZoneColor, ZoneId, BASE_ZONES};

const CYCLE_PERIOD: Duration = Duration::from_secs(4);
const WAVE_PERIOD: Duration = Duration::from_secs(3);
const BREATHING_PERIOD: Duration = Duration::from_millis(2400);
const BLINK_PERIOD: Duration = Duration::from_millis(1000);

pub struct LightingAnimator {
    config: LightingConfig,
    capabilities: KeyboardLightingCapabilities,
    started_at: Instant,
}

impl LightingAnimator {
    pub fn new(
        config: LightingConfig,
        capabilities: KeyboardLightingCapabilities,
        started_at: Instant,
    ) -> Self {
        Self {
            config,
            capabilities,
            started_at,
        }
    }

    pub fn update(
        &mut self,
        config: LightingConfig,
        capabilities: KeyboardLightingCapabilities,
        now: Instant,
    ) -> bool {
        if self.config == config && self.capabilities == capabilities {
            return false;
        }
        self.config = config;
        self.capabilities = capabilities;
        self.started_at = now;
        true
    }

    pub fn config(&self) -> &LightingConfig {
        &self.config
    }

    pub fn is_dynamic(&self) -> bool {
        self.config.mode != Mode::Custom
    }

    pub fn frame(&self, now: Instant) -> LightingFrame {
        let elapsed = now.saturating_duration_since(self.started_at);
        let zones = active_zones(&self.config, self.capabilities);
        let colors = zones
            .into_iter()
            .map(|zone| ZoneColor {
                zone,
                rgb: self.color_for_zone(zone, elapsed),
            })
            .collect();
        LightingFrame { colors }
    }

    fn color_for_zone(&self, zone: ZoneId, elapsed: Duration) -> Rgb {
        match self.config.mode {
            Mode::Custom => self.config.color,
            Mode::Cycle => hsv_to_rgb(period_progress(elapsed, CYCLE_PERIOD), 1.0, 1.0),
            Mode::Wave => {
                let spatial_phase = match (self.capabilities.layout, zone) {
                    (KeyboardLightingLayout::ThreeZone, ZoneId::F1) => 1.0 / 3.0,
                    (KeyboardLightingLayout::ThreeZone, ZoneId::F2) => 2.0 / 3.0,
                    _ => 0.0,
                };
                hsv_to_rgb(
                    (period_progress(elapsed, WAVE_PERIOD) + spatial_phase).fract(),
                    1.0,
                    1.0,
                )
            }
            Mode::Breathing => {
                let wave = (period_progress(elapsed, BREATHING_PERIOD) * TAU - TAU / 4.0).sin();
                scale_rgb(self.config.color, 0.08 + (wave + 1.0) * 0.46)
            }
            Mode::Blink => {
                let progress = period_progress(elapsed, BLINK_PERIOD);
                let level = if progress < 0.45 {
                    1.0
                } else if progress < 0.55 {
                    1.0 - (progress - 0.45) * 10.0
                } else if progress < 0.95 {
                    0.0
                } else {
                    (progress - 0.95) * 20.0
                };
                scale_rgb(self.config.color, level)
            }
        }
    }
}

fn active_zones(
    config: &LightingConfig,
    capabilities: KeyboardLightingCapabilities,
) -> Vec<ZoneId> {
    if matches!(
        capabilities.layout,
        KeyboardLightingLayout::Unsupported
            | KeyboardLightingLayout::White
            | KeyboardLightingLayout::PerKey
    ) {
        return Vec::new();
    }

    let mut zones = match capabilities.layout {
        KeyboardLightingLayout::SingleZone => vec![ZoneId::F0],
        KeyboardLightingLayout::ThreeZone | KeyboardLightingLayout::Unknown => BASE_ZONES
            .into_iter()
            .filter(|zone| config.zones.contains(zone))
            .collect(),
        KeyboardLightingLayout::Unsupported
        | KeyboardLightingLayout::White
        | KeyboardLightingLayout::PerKey => Vec::new(),
    };
    if zones.is_empty() {
        zones.extend(BASE_ZONES);
    }
    if capabilities.lightbar == Some(true) && config.zones.contains(&ZoneId::F3) {
        zones.push(ZoneId::F3);
    }
    if capabilities.logo == Some(true) && config.zones.contains(&ZoneId::F6) {
        zones.push(ZoneId::F6);
    }
    zones
}

fn period_progress(elapsed: Duration, period: Duration) -> f32 {
    (elapsed.as_secs_f32() / period.as_secs_f32()).fract()
}

fn scale_rgb(color: Rgb, level: f32) -> Rgb {
    let level = level.clamp(0.0, 1.0);
    Rgb {
        r: (f32::from(color.r) * level).round() as u8,
        g: (f32::from(color.g) * level).round() as u8,
        b: (f32::from(color.b) * level).round() as u8,
    }
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Rgb {
    let hue = hue.rem_euclid(1.0) * 6.0;
    let sector = hue.floor() as u8;
    let fraction = hue - f32::from(sector);
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - fraction * saturation);
    let t = value * (1.0 - (1.0 - fraction) * saturation);
    let (r, g, b) = match sector {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    Rgb {
        r: (r * 255.0).round() as u8,
        g: (g * 255.0).round() as u8,
        b: (b * 255.0).round() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(mode: Mode) -> LightingConfig {
        LightingConfig {
            mode,
            brightness_percent: 100,
            color: Rgb {
                r: 200,
                g: 100,
                b: 50,
            },
            zones: BASE_ZONES.to_vec(),
        }
    }

    #[test]
    fn single_zone_uses_one_firmware_write_per_frame() {
        let start = Instant::now();
        let animator = LightingAnimator::new(
            config(Mode::Custom),
            KeyboardLightingCapabilities {
                layout: KeyboardLightingLayout::SingleZone,
                logo: Some(false),
                lightbar: Some(false),
            },
            start,
        );

        assert_eq!(
            animator.frame(start).colors,
            vec![ZoneColor {
                zone: ZoneId::F0,
                rgb: Rgb {
                    r: 200,
                    g: 100,
                    b: 50
                }
            }]
        );
    }

    #[test]
    fn wave_has_spatial_phase_only_on_three_zone_keyboards() {
        let start = Instant::now();
        let animator = LightingAnimator::new(
            config(Mode::Wave),
            KeyboardLightingCapabilities {
                layout: KeyboardLightingLayout::ThreeZone,
                logo: Some(false),
                lightbar: Some(false),
            },
            start,
        );
        let frame = animator.frame(start);

        assert_eq!(frame.colors[0].rgb, Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(frame.colors[1].rgb, Rgb { r: 0, g: 255, b: 0 });
        assert_eq!(frame.colors[2].rgb, Rgb { r: 0, g: 0, b: 255 });
    }

    #[test]
    fn unsupported_keyboard_does_not_emit_rgb_writes() {
        let start = Instant::now();
        let animator = LightingAnimator::new(
            config(Mode::Cycle),
            KeyboardLightingCapabilities {
                layout: KeyboardLightingLayout::White,
                logo: None,
                lightbar: None,
            },
            start,
        );

        assert!(animator.frame(start).colors.is_empty());
    }
}
