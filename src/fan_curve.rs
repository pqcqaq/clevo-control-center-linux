use serde::{Deserialize, Serialize};

use crate::preferences::UiLanguage;

pub const FAN_CURVE_COUNT: usize = 3;
pub const FAN_CURVE_POINT_COUNT: usize = 4;
pub const FAN_CURVE_MIN_TEMP: u8 = 30;
pub const FAN_CURVE_MAX_TEMP: u8 = 100;
pub const FAN_CURVE_MIN_DUTY: u8 = 0;
pub const FAN_CURVE_MAX_DUTY: u8 = 100;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum FanCurveChannel {
    Cpu,
    Gpu,
}

impl FanCurveChannel {
    pub fn localized_label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::Cpu => language.pick("CPU 曲线", "CPU curve"),
            Self::Gpu => language.pick("GPU 曲线", "GPU curve"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanCurveSelection {
    pub profile: usize,
    pub channel: FanCurveChannel,
    pub point: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FanCurvePoint {
    pub temp_celsius: u8,
    pub duty_percent: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FanCurve {
    #[serde(default = "default_curve_points")]
    pub points: Vec<FanCurvePoint>,
}

impl Default for FanCurve {
    fn default() -> Self {
        Self {
            points: default_curve_points(),
        }
    }
}

impl FanCurve {
    pub fn sanitized(mut self) -> Self {
        if self.points.is_empty() {
            self.points = default_curve_points();
        }

        self.points.truncate(FAN_CURVE_POINT_COUNT);
        while self.points.len() < FAN_CURVE_POINT_COUNT {
            self.points.push(default_curve_points()[self.points.len()]);
        }

        self.points.sort_by_key(|point| point.temp_celsius);
        for point in &mut self.points {
            point.temp_celsius = point
                .temp_celsius
                .clamp(FAN_CURVE_MIN_TEMP, FAN_CURVE_MAX_TEMP);
            point.duty_percent = point
                .duty_percent
                .clamp(FAN_CURVE_MIN_DUTY, FAN_CURVE_MAX_DUTY);
        }
        for index in 1..self.points.len() {
            let min_temp = self.points[index - 1].temp_celsius.saturating_add(1);
            self.points[index].temp_celsius = self.points[index].temp_celsius.max(min_temp);
        }
        if let Some(last) = self.points.last_mut() {
            last.temp_celsius = last.temp_celsius.min(FAN_CURVE_MAX_TEMP);
        }
        for index in (0..self.points.len().saturating_sub(1)).rev() {
            let max_temp = self.points[index + 1].temp_celsius.saturating_sub(1);
            self.points[index].temp_celsius = self.points[index].temp_celsius.min(max_temp);
        }
        for index in 1..self.points.len() {
            let min_duty = self.points[index - 1].duty_percent;
            self.points[index].duty_percent = self.points[index].duty_percent.max(min_duty);
        }
        self
    }

    pub fn set_point(&mut self, index: usize, temp_celsius: u8, duty_percent: u8) {
        if index == 0 || index + 1 >= self.points.len() {
            return;
        }

        let lower = if index == 0 {
            FAN_CURVE_MIN_TEMP
        } else {
            self.points[index - 1].temp_celsius.saturating_add(1)
        };
        let upper = self
            .points
            .get(index + 1)
            .map(|point| point.temp_celsius.saturating_sub(1))
            .unwrap_or(FAN_CURVE_MAX_TEMP);
        let lower_duty = if index == 0 {
            FAN_CURVE_MIN_DUTY
        } else {
            self.points[index - 1].duty_percent
        };
        let upper_duty = self
            .points
            .get(index + 1)
            .map(|point| point.duty_percent)
            .unwrap_or(FAN_CURVE_MAX_DUTY)
            .max(lower_duty);

        self.points[index] = FanCurvePoint {
            temp_celsius: temp_celsius.clamp(lower, upper),
            duty_percent: duty_percent.clamp(lower_duty, upper_duty),
        };
    }

    pub fn apply_firmware_anchor(&mut self, first: FanCurvePoint) {
        *self = self.clone().sanitized();
        let middle_1 = self.points[1];
        let middle_2 = self.points[2];
        self.points[0] = first;
        self.points[3] = FanCurvePoint {
            temp_celsius: FAN_CURVE_MAX_TEMP,
            duty_percent: FAN_CURVE_MAX_DUTY,
        };

        self.points[1].temp_celsius = middle_1
            .temp_celsius
            .clamp(first.temp_celsius.saturating_add(1), FAN_CURVE_MAX_TEMP - 2);
        self.points[2].temp_celsius = middle_2.temp_celsius.clamp(
            self.points[1].temp_celsius.saturating_add(1),
            FAN_CURVE_MAX_TEMP - 1,
        );
        self.points[1].duty_percent = middle_1
            .duty_percent
            .clamp(first.duty_percent, FAN_CURVE_MAX_DUTY);
        self.points[2].duty_percent = middle_2
            .duty_percent
            .clamp(self.points[1].duty_percent, FAN_CURVE_MAX_DUTY);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FanCurveProfile {
    pub cpu: FanCurve,
    pub gpu: FanCurve,
}

impl FanCurveProfile {
    pub fn sanitized(self) -> Self {
        Self {
            cpu: self.cpu.sanitized(),
            gpu: self.gpu.sanitized(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FanCurveSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub selected_profile: Option<usize>,
    #[serde(default = "default_fan_curve_profiles")]
    pub profiles: Vec<FanCurveProfile>,
}

impl Default for FanCurveSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            selected_profile: None,
            profiles: default_fan_curve_profiles(),
        }
    }
}

impl FanCurveSettings {
    pub fn sanitized(mut self) -> Self {
        self.profiles.truncate(FAN_CURVE_COUNT);
        while self.profiles.len() < FAN_CURVE_COUNT {
            self.profiles
                .push(default_fan_curve_profiles()[self.profiles.len()].clone());
        }
        self.profiles = self
            .profiles
            .into_iter()
            .map(FanCurveProfile::sanitized)
            .collect();
        if !self.enabled || !matches!(self.selected_profile, Some(index) if index < FAN_CURVE_COUNT)
        {
            self.selected_profile = None;
        }
        self
    }

    pub fn localized_profile_label(index: usize, language: UiLanguage) -> &'static str {
        match index {
            0 => language.pick("曲线 1", "Curve 1"),
            1 => language.pick("曲线 2", "Curve 2"),
            2 => language.pick("曲线 3", "Curve 3"),
            _ => language.pick("曲线", "Curve"),
        }
    }

    pub fn mode_value(index: usize) -> &'static str {
        match index {
            0 => "curve1",
            1 => "curve2",
            2 => "curve3",
            _ => "curve",
        }
    }

    pub fn mode_index(value: &str) -> Option<usize> {
        match value {
            "curve1" => Some(0),
            "curve2" => Some(1),
            "curve3" => Some(2),
            _ => None,
        }
    }

    pub fn apply_firmware_anchors(&mut self, cpu: FanCurvePoint, gpu: FanCurvePoint) {
        for profile in &mut self.profiles {
            profile.cpu.apply_firmware_anchor(cpu);
            profile.gpu.apply_firmware_anchor(gpu);
        }
    }
}

pub fn default_fan_curve_profiles() -> Vec<FanCurveProfile> {
    vec![
        FanCurveProfile {
            cpu: curve(&[(40, 32), (58, 42), (78, 72), (100, 100)]),
            gpu: curve(&[(40, 32), (60, 44), (80, 74), (100, 100)]),
        },
        FanCurveProfile {
            cpu: curve(&[(40, 32), (55, 52), (72, 78), (100, 100)]),
            gpu: curve(&[(40, 32), (58, 54), (74, 80), (100, 100)]),
        },
        FanCurveProfile {
            cpu: curve(&[(40, 32), (62, 35), (82, 66), (100, 100)]),
            gpu: curve(&[(40, 32), (64, 38), (84, 68), (100, 100)]),
        },
    ]
}

fn default_curve_points() -> Vec<FanCurvePoint> {
    curve(&[(40, 32), (58, 42), (78, 72), (100, 100)]).points
}

fn curve(points: &[(u8, u8)]) -> FanCurve {
    FanCurve {
        points: points
            .iter()
            .map(|(temp_celsius, duty_percent)| FanCurvePoint {
                temp_celsius: *temp_celsius,
                duty_percent: *duty_percent,
            })
            .collect(),
    }
    .sanitized()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_curve_clamps_and_keeps_points_ordered() {
        let curve = FanCurve {
            points: vec![
                FanCurvePoint {
                    temp_celsius: 120,
                    duty_percent: 140,
                },
                FanCurvePoint {
                    temp_celsius: 20,
                    duty_percent: 1,
                },
                FanCurvePoint {
                    temp_celsius: 60,
                    duty_percent: 50,
                },
                FanCurvePoint {
                    temp_celsius: 60,
                    duty_percent: 55,
                },
            ],
        }
        .sanitized();

        assert_eq!(curve.points[0].temp_celsius, 30);
        assert_eq!(curve.points[0].duty_percent, 1);
        assert!(curve
            .points
            .windows(2)
            .all(|pair| pair[0].temp_celsius < pair[1].temp_celsius));
        assert_eq!(curve.points[3].duty_percent, FAN_CURVE_MAX_DUTY);
    }

    #[test]
    fn set_point_does_not_cross_neighbors() {
        let mut curve = FanCurve::default();

        curve.set_point(1, 99, 120);

        assert_eq!(
            curve.points[1].temp_celsius,
            curve.points[2].temp_celsius - 1
        );
        assert_eq!(curve.points[1].duty_percent, curve.points[2].duty_percent);
    }

    #[test]
    fn fixed_endpoints_cannot_be_edited() {
        let mut curve = FanCurve::default();
        let first = curve.points[0];
        let last = curve.points[3];

        curve.set_point(0, 55, 80);
        curve.set_point(3, 90, 90);

        assert_eq!(curve.points[0], first);
        assert_eq!(curve.points[3], last);
    }

    #[test]
    fn firmware_anchor_replaces_legacy_endpoints_and_clamps_middle_points() {
        let mut curve = FanCurve {
            points: vec![
                FanCurvePoint {
                    temp_celsius: 30,
                    duty_percent: 10,
                },
                FanCurvePoint {
                    temp_celsius: 38,
                    duty_percent: 20,
                },
                FanCurvePoint {
                    temp_celsius: 78,
                    duty_percent: 70,
                },
                FanCurvePoint {
                    temp_celsius: 95,
                    duty_percent: 90,
                },
            ],
        };

        curve.apply_firmware_anchor(FanCurvePoint {
            temp_celsius: 40,
            duty_percent: 32,
        });

        assert_eq!(curve.points[0].temp_celsius, 40);
        assert_eq!(curve.points[0].duty_percent, 32);
        assert_eq!(curve.points[1].temp_celsius, 41);
        assert_eq!(curve.points[1].duty_percent, 32);
        assert_eq!(curve.points[3].temp_celsius, 100);
        assert_eq!(curve.points[3].duty_percent, 100);
        assert!(curve
            .points
            .windows(2)
            .all(|pair| pair[0].temp_celsius < pair[1].temp_celsius));
        assert!(curve
            .points
            .windows(2)
            .all(|pair| pair[0].duty_percent <= pair[1].duty_percent));
    }

    #[test]
    fn fan_curve_keeps_duty_monotonic() {
        let curve = FanCurve {
            points: vec![
                FanCurvePoint {
                    temp_celsius: 40,
                    duty_percent: 70,
                },
                FanCurvePoint {
                    temp_celsius: 55,
                    duty_percent: 30,
                },
                FanCurvePoint {
                    temp_celsius: 70,
                    duty_percent: 80,
                },
                FanCurvePoint {
                    temp_celsius: 95,
                    duty_percent: 60,
                },
            ],
        }
        .sanitized();

        assert!(curve
            .points
            .windows(2)
            .all(|pair| pair[0].duty_percent <= pair[1].duty_percent));
    }

    #[test]
    fn fan_curve_settings_sanitize_keeps_three_profiles() {
        let settings = FanCurveSettings {
            enabled: true,
            selected_profile: Some(8),
            profiles: Vec::new(),
        }
        .sanitized();

        assert_eq!(settings.profiles.len(), FAN_CURVE_COUNT);
        assert_eq!(settings.selected_profile, None);
    }
}
