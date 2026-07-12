use eframe::egui::{Color32, Context, Stroke};

use crate::preferences::ThemeColor;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub accent: Color32,
    pub strong: Color32,
    pub surface: Color32,
    pub selected_surface: Color32,
    pub border: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub bright: Color32,
}

pub(super) fn palette(theme: ThemeColor) -> Palette {
    match theme {
        ThemeColor::Amber => Palette {
            accent: Color32::from_rgb(231, 166, 84),
            strong: Color32::from_rgb(235, 168, 80),
            surface: Color32::from_rgb(58, 43, 24),
            selected_surface: Color32::from_rgb(74, 52, 27),
            border: Color32::from_rgb(221, 164, 91),
            text: Color32::from_rgb(255, 236, 201),
            dim: Color32::from_rgb(198, 143, 80),
            bright: Color32::from_rgb(255, 206, 132),
        },
        ThemeColor::Cyan => Palette {
            accent: Color32::from_rgb(74, 204, 214),
            strong: Color32::from_rgb(76, 217, 226),
            surface: Color32::from_rgb(24, 54, 58),
            selected_surface: Color32::from_rgb(26, 66, 71),
            border: Color32::from_rgb(81, 202, 211),
            text: Color32::from_rgb(210, 248, 250),
            dim: Color32::from_rgb(65, 164, 172),
            bright: Color32::from_rgb(139, 239, 244),
        },
        ThemeColor::Emerald => Palette {
            accent: Color32::from_rgb(79, 193, 132),
            strong: Color32::from_rgb(84, 207, 141),
            surface: Color32::from_rgb(25, 53, 39),
            selected_surface: Color32::from_rgb(27, 66, 47),
            border: Color32::from_rgb(84, 190, 132),
            text: Color32::from_rgb(218, 247, 229),
            dim: Color32::from_rgb(67, 154, 108),
            bright: Color32::from_rgb(145, 230, 179),
        },
        ThemeColor::Rose => Palette {
            accent: Color32::from_rgb(218, 103, 145),
            strong: Color32::from_rgb(229, 109, 153),
            surface: Color32::from_rgb(61, 30, 43),
            selected_surface: Color32::from_rgb(75, 33, 50),
            border: Color32::from_rgb(213, 106, 145),
            text: Color32::from_rgb(255, 225, 236),
            dim: Color32::from_rgb(176, 82, 116),
            bright: Color32::from_rgb(245, 164, 193),
        },
    }
}

pub(crate) fn apply(ctx: &Context, theme: ThemeColor) {
    let palette = palette(theme);
    let mut visuals = ctx.style().visuals.clone();
    visuals.selection.bg_fill = palette.surface;
    visuals.selection.stroke = Stroke::new(1.0, palette.border);
    visuals.hyperlink_color = palette.accent;
    visuals.widgets.active.bg_fill = palette.selected_surface;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.hovered.bg_fill = mix(Color32::from_rgb(46, 43, 38), palette.surface, 0.45);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.border);
    ctx.set_visuals(visuals);
}

pub(super) fn mix(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        blend(from.r(), to.r()),
        blend(from.g(), to.g()),
        blend(from.b(), to.b()),
        blend(from.a(), to.a()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amber_palette_preserves_original_accent() {
        let amber = palette(ThemeColor::Amber);
        assert_eq!(amber.accent, Color32::from_rgb(231, 166, 84));
        assert_eq!(amber.border, Color32::from_rgb(221, 164, 91));
    }

    #[test]
    fn color_mix_clamps_interpolation() {
        let from = Color32::from_rgb(10, 20, 30);
        let to = Color32::from_rgb(110, 120, 130);
        assert_eq!(mix(from, to, -1.0), from);
        assert_eq!(mix(from, to, 2.0), to);
        assert_eq!(mix(from, to, 0.5), Color32::from_rgb(60, 70, 80));
    }
}
