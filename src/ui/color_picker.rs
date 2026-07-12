use std::io;
use std::process::Command;

use eframe::egui::{vec2, Color32, Sense, Stroke, Ui};

use super::app::ClevoLedApp;
use super::theme;
use crate::model::{Mode, Rgb};

pub(super) fn color_swatch(ui: &mut Ui, app: &mut ClevoLedApp) {
    let size = vec2(62.0, 62.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 23.0;
    painter.circle_filled(center, radius + 3.0, Color32::from_rgb(15, 14, 13));
    painter.circle_stroke(
        center,
        radius + 4.0,
        Stroke::new(1.0, Color32::from_rgb(70, 64, 54)),
    );
    painter.circle_filled(center, radius, rgb_color32(app.f0_color));

    if response.hovered() && matches!(app.mode, Mode::Custom | Mode::Breathing) {
        painter.circle_stroke(
            center,
            radius + 6.0,
            Stroke::new(1.5, theme::palette(app.theme_color).accent),
        );
    }

    if response.clicked() && matches!(app.mode, Mode::Custom | Mode::Breathing) {
        match open_native_color_picker(app.f0_color, app.language) {
            Ok(Some(rgb)) => {
                app.f0_color = rgb;
                app.mark_settings_dirty();
                app.persist_settings_if_due(true);
            }
            Ok(None) => {}
            Err(err) => {
                app.last_error = Some(err.to_string());
                eprintln!("Failed to open color picker: {err}");
            }
        }
    }
}

fn rgb_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

fn open_native_color_picker(
    current: Rgb,
    language: crate::preferences::UiLanguage,
) -> io::Result<Option<Rgb>> {
    let current_hex = format!("#{:02x}{:02x}{:02x}", current.r, current.g, current.b);

    if command_exists("zenity") {
        let output = Command::new("zenity")
            .args([
                "--color-selection",
                "--show-palette",
                "--color",
                &current_hex,
            ])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(
            &output.stdout,
        )));
    }

    if command_exists("kdialog") {
        let output = Command::new("kdialog")
            .args(["--getcolor", &current_hex])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(
            &output.stdout,
        )));
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        language.pick(
            "需要安装 zenity 或 kdialog 才能弹出系统调色盘",
            "Install zenity or kdialog to open the system color picker",
        ),
    ))
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {command} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn parse_color_picker_output(output: &str) -> Option<Rgb> {
    let text = output.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex_rgb(hex);
    }

    if let Some(body) = text
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let mut channels = body.split(',').map(|part| {
            part.trim()
                .parse::<u16>()
                .ok()
                .and_then(|value| u8::try_from(value).ok())
        });
        let rgb = Rgb {
            r: channels.next()??,
            g: channels.next()??,
            b: channels.next()??,
        };
        if channels.next().is_none() {
            return Some(rgb);
        }
    }

    None
}

fn parse_hex_rgb(hex: &str) -> Option<Rgb> {
    if hex.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(Rgb {
        r: ((value >> 16) & 0xff) as u8,
        g: ((value >> 8) & 0xff) as u8,
        b: (value & 0xff) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_color_picker_outputs() {
        assert_eq!(
            parse_color_picker_output("#0c2238\n"),
            Some(Rgb {
                r: 12,
                g: 34,
                b: 56
            })
        );
        assert_eq!(
            parse_color_picker_output("rgb(12,34,56)\n"),
            Some(Rgb {
                r: 12,
                g: 34,
                b: 56
            })
        );
        assert_eq!(parse_color_picker_output(""), None);
        assert_eq!(parse_color_picker_output("rgb(256,34,56)"), None);
        assert_eq!(parse_color_picker_output("rgb(12,34,56,78)"), None);
    }
}
