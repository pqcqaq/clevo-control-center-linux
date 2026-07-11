use serde::{Deserialize, Serialize};

pub const BASE_ZONES: [ZoneId; 3] = [ZoneId::F0, ZoneId::F1, ZoneId::F2];
pub const ALL_ZONES: [ZoneId; 7] = [
    ZoneId::F0,
    ZoneId::F1,
    ZoneId::F2,
    ZoneId::F3,
    ZoneId::F4,
    ZoneId::F5,
    ZoneId::F6,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ZoneId {
    #[serde(rename = "f0")]
    F0,
    #[serde(rename = "f1")]
    F1,
    #[serde(rename = "f2")]
    F2,
    #[serde(rename = "f3")]
    F3,
    #[serde(rename = "f4")]
    F4,
    #[serde(rename = "f5")]
    F5,
    #[serde(rename = "f6")]
    F6,
}

impl ZoneId {
    pub fn proc_code(self) -> &'static str {
        match self {
            Self::F0 => "f0",
            Self::F1 => "f1",
            Self::F2 => "f2",
            Self::F3 => "f3",
            Self::F4 => "f4",
            Self::F5 => "f5",
            Self::F6 => "f6",
        }
    }

    pub fn label(self) -> &'static str {
        self.proc_code()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub fn hex_lower(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "cycle")]
    Cycle,
    #[serde(rename = "wave", alias = "chase")]
    Wave,
    #[serde(rename = "blink")]
    Blink,
    #[serde(rename = "breathing")]
    Breathing,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Custom => "自定义",
            Self::Cycle => "循环",
            Self::Wave => "波浪",
            Self::Blink => "闪烁",
            Self::Breathing => "呼吸",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Custom,
            Self::Cycle,
            Self::Wave,
            Self::Blink,
            Self::Breathing,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlPage {
    Overview,
    Lighting,
    Fan,
    Battery,
    Gpu,
    #[cfg(debug_assertions)]
    Diagnostics,
    Settings,
    #[cfg(debug_assertions)]
    Advanced,
}

impl ControlPage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "总览",
            Self::Lighting => "灯光",
            Self::Fan => "风扇",
            Self::Battery => "电池",
            Self::Gpu => "显卡",
            #[cfg(debug_assertions)]
            Self::Diagnostics => "诊断",
            Self::Settings => "设置",
            #[cfg(debug_assertions)]
            Self::Advanced => "高级",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Overview,
            Self::Lighting,
            Self::Fan,
            Self::Battery,
            Self::Gpu,
            #[cfg(debug_assertions)]
            Self::Diagnostics,
            Self::Settings,
            #[cfg(debug_assertions)]
            Self::Advanced,
        ]
    }
}

#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvancedTab {
    Fans,
    Temperatures,
    Other,
}

#[cfg(debug_assertions)]
impl AdvancedTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fans => "风扇信息",
            Self::Temperatures => "温度信息",
            Self::Other => "其他信息",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Fans, Self::Temperatures, Self::Other]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZoneColor {
    pub zone: ZoneId,
    pub rgb: Rgb,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingConfig {
    pub mode: Mode,
    pub brightness_percent: u8,
    pub color: Rgb,
    pub zones: Vec<ZoneId>,
}

pub fn default_zones() -> Vec<ZoneId> {
    BASE_ZONES.to_vec()
}

pub fn normalize_zones(zones: &[ZoneId]) -> Vec<ZoneId> {
    let normalized = ALL_ZONES
        .into_iter()
        .filter(|zone| zones.contains(zone))
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        default_zones()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_pages_follow_lighting() {
        let pages = ControlPage::all();
        let lighting_index = pages
            .iter()
            .position(|page| *page == ControlPage::Lighting)
            .unwrap();
        assert_eq!(pages.get(lighting_index + 1), Some(&ControlPage::Fan));
        assert_eq!(pages.get(lighting_index + 2), Some(&ControlPage::Battery));
        assert_eq!(pages.get(lighting_index + 3), Some(&ControlPage::Gpu));
        #[cfg(debug_assertions)]
        assert_eq!(
            pages.get(lighting_index + 4),
            Some(&ControlPage::Diagnostics)
        );
        #[cfg(not(debug_assertions))]
        assert_eq!(pages.get(lighting_index + 4), Some(&ControlPage::Settings));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn advanced_page_is_after_settings() {
        let pages = ControlPage::all();
        let settings_index = pages
            .iter()
            .position(|page| *page == ControlPage::Settings)
            .unwrap();
        assert_eq!(pages.get(settings_index + 1), Some(&ControlPage::Advanced));
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_navigation_ends_at_settings() {
        assert_eq!(ControlPage::all().last(), Some(&ControlPage::Settings));
    }
}
