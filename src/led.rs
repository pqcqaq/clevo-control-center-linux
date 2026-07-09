use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::{ZoneColor, BASE_ZONES};

const DEFAULT_PROC_PATH: &str = "/proc/clevo_kbd_led";

pub struct LedWriter {
    proc_path: PathBuf,
}

impl LedWriter {
    pub fn new() -> Self {
        let proc_path = std::env::var_os("CLEVO_KBD_LED_PROC")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_PROC_PATH));
        Self { proc_path }
    }

    pub fn ready(&self) -> bool {
        self.proc_path.exists()
            && fs::OpenOptions::new()
                .write(true)
                .open(&self.proc_path)
                .is_ok()
    }

    pub fn write(&self, colors: &[ZoneColor]) -> io::Result<()> {
        if colors.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no zones"));
        }

        for command in commands_for_colors(colors) {
            fs::write(&self.proc_path, command)?;
        }
        Ok(())
    }

    pub fn proc_path(&self) -> &Path {
        &self.proc_path
    }
}

pub fn commands_for_colors(colors: &[ZoneColor]) -> Vec<String> {
    if colors.len() == 3
        && BASE_ZONES
            .iter()
            .all(|zone| colors.iter().any(|color| color.zone == *zone))
        && colors.iter().all(|color| color.rgb == colors[0].rgb)
    {
        return vec![format!("{}\n", colors[0].rgb.hex_lower())];
    }

    colors
        .iter()
        .map(|color| format!("{} {}\n", color.zone.proc_code(), color.rgb.hex_lower()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Rgb, ZoneId, BASE_ZONES};

    #[test]
    fn serializes_base_zones_same_color_as_short_command() {
        let colors = BASE_ZONES
            .into_iter()
            .map(|zone| ZoneColor {
                zone,
                rgb: Rgb { r: 255, g: 0, b: 0 },
            })
            .collect::<Vec<_>>();

        assert_eq!(commands_for_colors(&colors), vec!["ff0000\n"]);
    }

    #[test]
    fn serializes_mixed_zone_commands() {
        let colors = vec![
            ZoneColor {
                zone: ZoneId::F0,
                rgb: Rgb { r: 255, g: 0, b: 0 },
            },
            ZoneColor {
                zone: ZoneId::F2,
                rgb: Rgb { r: 0, g: 0, b: 255 },
            },
        ];

        assert_eq!(
            commands_for_colors(&colors),
            vec!["f0 ff0000\n", "f2 0000ff\n"]
        );
    }
}
