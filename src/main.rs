use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, pos2, vec2, Align2, Button, CentralPanel, Color32, ComboBox, Context,
    FontData, FontDefinitions, FontFamily, FontId, Frame, RichText, Sense, Slider, Stroke, Ui,
    ViewportBuilder, ViewportCommand,
};
use serde::{Deserialize, Serialize};

const DEFAULT_PROC_PATH: &str = "/proc/clevo_kbd_led";
const SETTINGS_FILE: &str = "settings.json";
const SERVICE_PID_FILE: &str = "clevo-keyboard-led.pid";
const SERVICE_LOG_FILE: &str = "clevo-keyboard-led.service.log";
const BASE_ZONES: [ZoneId; 3] = [ZoneId::F0, ZoneId::F1, ZoneId::F2];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum ZoneId {
    F0,
    F1,
    F2,
}

impl ZoneId {
    fn proc_code(self) -> &'static str {
        match self {
            Self::F0 => "f0",
            Self::F1 => "f1",
            Self::F2 => "f2",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    fn color32(self) -> Color32 {
        Color32::from_rgb(self.r, self.g, self.b)
    }

    fn hex_lower(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Mode {
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "cycle")]
    Cycle,
    #[serde(rename = "chase")]
    Chase,
    #[serde(rename = "blink")]
    Blink,
    #[serde(rename = "breathing")]
    Breathing,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Custom => "自定义",
            Self::Cycle => "循环",
            Self::Chase => "追逐",
            Self::Blink => "闪烁",
            Self::Breathing => "呼吸",
        }
    }

    fn all() -> &'static [Self] {
        &[
            Self::Custom,
            Self::Cycle,
            Self::Chase,
            Self::Blink,
            Self::Breathing,
        ]
    }
}

#[derive(Clone)]
struct ZoneColor {
    zone: ZoneId,
    rgb: Rgb,
}

struct LedWriter {
    proc_path: PathBuf,
}

impl LedWriter {
    fn new() -> Self {
        let proc_path = std::env::var_os("CLEVO_KBD_LED_PROC")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_PROC_PATH));
        Self { proc_path }
    }

    fn ready(&self) -> bool {
        self.proc_path.exists()
            && fs::OpenOptions::new()
                .write(true)
                .open(&self.proc_path)
                .is_ok()
    }

    fn write(&self, colors: &[ZoneColor]) -> io::Result<()> {
        if colors.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no zones"));
        }

        for command in commands_for_colors(colors) {
            fs::write(&self.proc_path, command)?;
        }
        Ok(())
    }

    fn proc_path(&self) -> &Path {
        &self.proc_path
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Settings {
    mode: Mode,
    speed: u8,
    brightness: u8,
    running: bool,
    f0_color: Rgb,
    window_pos: Option<[f32; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Custom,
            speed: 36,
            brightness: 100,
            running: false,
            f0_color: Rgb::WHITE,
            window_pos: None,
        }
    }
}

impl Settings {
    fn sanitized(mut self) -> Self {
        self.speed = self.speed.clamp(1, 100);
        self.brightness = self.brightness.clamp(1, 100);
        if self.mode == Mode::Custom {
            self.running = false;
        }
        if let Some([x, y]) = self.window_pos {
            if !x.is_finite() || !y.is_finite() {
                self.window_pos = None;
            }
        }
        self
    }
}

fn settings_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(SETTINGS_FILE)
}

fn load_settings(path: &Path) -> Settings {
    match fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
    {
        Some(settings) => settings.sanitized(),
        None => Settings::default(),
    }
}

fn atomic_write_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, format!("{json}\n"))?;
    fs::rename(tmp_path, path)
}

fn commands_for_colors(colors: &[ZoneColor]) -> Vec<String> {
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

struct ClevoLedApp {
    writer: LedWriter,
    settings_path: PathBuf,
    mode: Mode,
    speed: u8,
    brightness: u8,
    running: bool,
    f0_color: Rgb,
    last_error: Option<String>,
    window_pos: Option<[f32; 2]>,
    dirty_settings: bool,
    last_settings_save: Instant,
}

impl ClevoLedApp {
    fn new(settings_path: PathBuf, settings: Settings) -> Self {
        let writer = LedWriter::new();
        if !writer.ready() {
            eprintln!("Keyboard LED interface is not writable: {}", writer.proc_path().display());
        }

        Self {
            writer,
            settings_path,
            mode: settings.mode,
            speed: settings.speed,
            brightness: settings.brightness,
            running: settings.running,
            f0_color: settings.f0_color,
            last_error: None,
            window_pos: settings.window_pos,
            dirty_settings: false,
            last_settings_save: Instant::now(),
        }
    }

    fn toggle(&mut self) {
        if self.mode == Mode::Custom {
            return;
        }

        self.running = !self.running;
        self.mark_settings_dirty();
        self.persist_settings_if_due(true);
    }

    fn write_f0(&mut self, rgb: Rgb) {
        if let Err(err) = self.writer.write(&[ZoneColor {
            zone: ZoneId::F0,
            rgb,
        }]) {
            self.last_error = Some(err.to_string());
            self.running = false;
            eprintln!("Failed to write f0 color: {err}");
        }
    }

    fn mark_settings_dirty(&mut self) {
        self.dirty_settings = true;
    }

    fn update_window_position(&mut self, ctx: &Context) {
        let position = ctx.input(|input| {
            input
                .viewport()
                .outer_rect
                .map(|rect| [rect.min.x, rect.min.y])
        });

        if let Some(position) = position {
            let changed = self
                .window_pos
                .map(|old| {
                    (old[0] - position[0]).abs() > 0.5
                        || (old[1] - position[1]).abs() > 0.5
                })
                .unwrap_or(true);
            if changed {
                self.window_pos = Some(position);
                self.mark_settings_dirty();
            }
        }
    }

    fn persist_settings_if_due(&mut self, force: bool) {
        if !self.dirty_settings && !force {
            return;
        }
        if !force && self.last_settings_save.elapsed() < Duration::from_millis(700) {
            return;
        }
        self.persist_settings();
    }

    fn persist_settings(&mut self) {
        let settings = Settings {
            mode: self.mode,
            speed: self.speed,
            brightness: self.brightness,
            running: self.running && self.mode != Mode::Custom,
            f0_color: self.f0_color,
            window_pos: self.window_pos,
        };

        match serde_json::to_string_pretty(&settings)
            .map_err(io::Error::other)
            .and_then(|_| atomic_write_settings(&self.settings_path, &settings))
        {
            Ok(()) => {
                self.dirty_settings = false;
                self.last_settings_save = Instant::now();
            }
            Err(err) => {
                eprintln!("Failed to write {}: {err}", self.settings_path.display());
                self.last_settings_save = Instant::now();
            }
        }
    }
}

impl eframe::App for ClevoLedApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.update_window_position(ctx);

        CentralPanel::default()
            .frame(Frame::none().fill(Color32::from_rgb(16, 18, 22)))
            .show(ctx, |ui| {
                custom_title_bar(ui, ctx);
                ui.add_space(2.0);
                main_controls(ui, self);
            });

        self.persist_settings_if_due(false);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_settings_if_due(true);
    }
}

fn custom_title_bar(ui: &mut Ui, ctx: &Context) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 18.0), Sense::click_and_drag());
    if response.drag_started() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(16, 18, 22));

    let close_rect = egui::Rect::from_min_size(pos2(rect.right() - 24.0, rect.top()), vec2(18.0, 18.0));
    let close_response = ui.interact(close_rect, ui.id().with("close"), Sense::click());
    let close_fill = if close_response.hovered() {
        Color32::from_rgb(92, 36, 42)
    } else {
        Color32::from_rgb(31, 34, 40)
    };
    painter.circle_filled(close_rect.center(), 5.5, close_fill);
    painter.text(
        close_rect.center(),
        Align2::CENTER_CENTER,
        "x",
        FontId::proportional(10.0),
        Color32::from_rgb(230, 230, 230),
    );
    if close_response.clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

fn main_controls(ui: &mut Ui, app: &mut ClevoLedApp) {
    let custom_mode = app.mode == Mode::Custom;
    Frame::none()
        .fill(Color32::from_rgb(21, 24, 29))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_min_height(86.0);
            ui.horizontal_centered(|ui| {
                color_swatch(ui, app);

                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.set_width(170.0);
                    ComboBox::from_id_salt("mode")
                        .width(170.0)
                        .selected_text(app.mode.label())
                        .show_ui(ui, |ui| {
                            for mode in Mode::all() {
                                let old_mode = app.mode;
                                let clicked = ui.selectable_value(&mut app.mode, *mode, mode.label()).clicked();
                                if app.mode != old_mode {
                                    if app.mode == Mode::Custom {
                                        app.running = false;
                                        app.write_f0(app.f0_color);
                                    }
                                    app.mark_settings_dirty();
                                    app.persist_settings_if_due(true);
                                }
                                if clicked && app.running && app.mode == Mode::Custom {
                                    app.write_f0(app.f0_color);
                                }
                            }
                        });

                    ui.add_space(8.0);
                    let speed_response = ui.add_enabled_ui(!custom_mode, |ui| {
                        ui.add_sized(
                            vec2(170.0, 18.0),
                            Slider::new(&mut app.speed, 1..=100).show_value(false),
                        )
                    }).inner;
                    if speed_response.changed() {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }

                    ui.add_space(6.0);
                    let brightness_response = ui.add_enabled_ui(!custom_mode, |ui| {
                        ui.add_sized(
                            vec2(170.0, 18.0),
                            Slider::new(&mut app.brightness, 1..=100).show_value(false),
                        )
                    }).inner;
                    if brightness_response.changed() {
                        app.mark_settings_dirty();
                        app.persist_settings_if_due(true);
                    }
                });

                ui.add_space(14.0);
                let label = if app.running { "结束" } else { "开始" };
                if ui
                    .add_enabled(!custom_mode, Button::new(RichText::new(label).size(16.0)).min_size(vec2(64.0, 42.0)))
                    .clicked()
                {
                    app.toggle();
                }
            });
        });
}

fn color_swatch(ui: &mut Ui, app: &mut ClevoLedApp) {
    let size = vec2(56.0, 56.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = 20.0;
    painter.circle_filled(center, radius + 3.0, Color32::from_rgb(9, 11, 14));
    painter.circle_stroke(center, radius + 4.0, Stroke::new(1.0, Color32::from_rgb(66, 72, 82)));
    painter.circle_filled(center, radius, app.f0_color.color32());

    if response.hovered() && app.mode == Mode::Custom {
        painter.circle_stroke(center, radius + 6.0, Stroke::new(1.5, Color32::from_rgb(118, 190, 255)));
    }

    if response.clicked() && app.mode == Mode::Custom {
        match open_native_color_picker(app.f0_color) {
            Ok(Some(rgb)) => {
                app.f0_color = rgb;
                app.mark_settings_dirty();
                app.persist_settings_if_due(true);
                app.write_f0(app.f0_color);
            }
            Ok(None) => {}
            Err(err) => {
                app.last_error = Some(err.to_string());
                eprintln!("Failed to open color picker: {err}");
            }
        }
    }
}

fn open_native_color_picker(current: Rgb) -> io::Result<Option<Rgb>> {
    let current_hex = format!("#{:02x}{:02x}{:02x}", current.r, current.g, current.b);

    if command_exists("zenity") {
        let output = Command::new("zenity")
            .args(["--color-selection", "--show-palette", "--color", &current_hex])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(&output.stdout)));
    }

    if command_exists("kdialog") {
        let output = Command::new("kdialog")
            .args(["--getcolor", &current_hex])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        return Ok(parse_color_picker_output(&String::from_utf8_lossy(&output.stdout)));
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "需要安装 zenity 或 kdialog 才能弹出系统调色盘",
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

    if let Some(body) = text.strip_prefix("rgb(").and_then(|value| value.strip_suffix(')')) {
        let values = body
            .split(',')
            .map(|part| part.trim().parse::<u16>().ok())
            .collect::<Option<Vec<_>>>()?;
        if values.len() == 3 && values.iter().all(|value| *value <= 255) {
            return Some(Rgb {
                r: values[0] as u8,
                g: values[1] as u8,
                b: values[2] as u8,
            });
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

fn ensure_service_running() {
    let pid_path = PathBuf::from(SERVICE_PID_FILE);
    if let Ok(pid_text) = fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_text.trim().parse::<u32>() {
            if process_is_running(pid) {
                return;
            }
        }
    }

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("Failed to locate executable for service: {err}");
            return;
        }
    };

    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(SERVICE_LOG_FILE);
    let stderr = log
        .ok()
        .map(Stdio::from)
        .unwrap_or_else(Stdio::null);

    match Command::new(exe)
        .arg("--service")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
    {
        Ok(child) => {
            if let Err(err) = fs::write(pid_path, format!("{}\n", child.id())) {
                eprintln!("Failed to write service pid file: {err}");
            }
        }
        Err(err) => eprintln!("Failed to start LED service: {err}"),
    }
}

fn process_is_running(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

fn service_loop(settings_path: PathBuf) -> ! {
    let writer = LedWriter::new();
    let mut phase = 0.0_f32;
    let mut last_static_color: Option<Rgb> = None;

    loop {
        let settings = load_settings(&settings_path);
        if settings.mode == Mode::Custom {
            if last_static_color != Some(settings.f0_color) {
                let _ = writer.write(&[ZoneColor {
                    zone: ZoneId::F0,
                    rgb: settings.f0_color,
                }]);
                last_static_color = Some(settings.f0_color);
            }
            thread::sleep(Duration::from_millis(180));
            continue;
        }

        last_static_color = None;
        if settings.running {
            phase = (phase + 0.0015 * settings.speed as f32).fract();
            let colors = colors_for_mode(settings.mode, phase, &settings);
            if let Err(err) = writer.write(&colors) {
                eprintln!("LED service write failed: {err}");
            }
            thread::sleep(tick_interval(settings.speed));
        } else {
            thread::sleep(Duration::from_millis(180));
        }
    }
}

fn colors_for_mode(mode: Mode, phase: f32, settings: &Settings) -> Vec<ZoneColor> {
    let brightness = settings.brightness as f32 / 100.0;
    match mode {
        Mode::Custom => vec![ZoneColor {
            zone: ZoneId::F0,
            rgb: settings.f0_color,
        }],
        Mode::Cycle => {
            let rgb = hsv_rgb(phase, 1.0, brightness);
            BASE_ZONES
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
        Mode::Chase => BASE_ZONES
            .into_iter()
            .enumerate()
            .map(|(index, zone)| ZoneColor {
                zone,
                rgb: hsv_rgb(phase + index as f32 / 3.0, 1.0, brightness),
            })
            .collect(),
        Mode::Blink => {
            let rgb = if (phase * 10.0) as i32 % 2 == 0 {
                scale_rgb(settings.f0_color, brightness)
            } else {
                Rgb::BLACK
            };
            BASE_ZONES
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
        Mode::Breathing => {
            let pulse = 0.12 + 0.88 * ((phase * std::f32::consts::TAU).sin() + 1.0) / 2.0;
            let rgb = scale_rgb(settings.f0_color, pulse * brightness);
            BASE_ZONES
                .into_iter()
                .map(|zone| ZoneColor { zone, rgb })
                .collect()
        }
    }
}

fn tick_interval(speed: u8) -> Duration {
    let millis = 180_u64.saturating_sub((speed as u64 * 135) / 100).max(25);
    Duration::from_millis(millis)
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

fn hsv_rgb(hue: f32, saturation: f32, value: f32) -> Rgb {
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

fn install_cjk_font(ctx: &Context) {
    const FONT_CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    ];

    let Some((path, bytes)) = FONT_CANDIDATES
        .iter()
        .find_map(|path| fs::read(path).ok().map(|bytes| (*path, bytes)))
    else {
        eprintln!("No CJK font found; Chinese text may not render correctly");
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk_fallback".to_owned(), FontData::from_owned(bytes));

    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk_fallback".to_owned());
    }

    ctx.set_fonts(fonts);
    eprintln!("Loaded CJK font: {path}");
}

fn main() -> eframe::Result {
    if std::env::args().any(|arg| arg == "--service") {
        service_loop(settings_path());
    }

    let settings_path = settings_path();
    let settings = load_settings(&settings_path);
    ensure_service_running();
    let mut viewport = ViewportBuilder::default()
        .with_inner_size([430.0, 130.0])
        .with_min_inner_size([430.0, 130.0])
        .with_max_inner_size([430.0, 130.0])
        .with_decorations(false)
        .with_resizable(false);

    if let Some([x, y]) = settings.window_pos {
        viewport = viewport.with_position(pos2(x, y));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(ClevoLedApp::new(settings_path, settings)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn hsv_cycle_starts_at_red() {
        assert_eq!(hsv_rgb(0.0, 1.0, 1.0), Rgb { r: 255, g: 0, b: 0 });
    }

    #[test]
    fn settings_json_roundtrips() {
        let settings = Settings {
            mode: Mode::Chase,
            speed: 72,
            brightness: 64,
            running: true,
            f0_color: Rgb { r: 12, g: 34, b: 56 },
            window_pos: Some([100.0, 200.0]),
        };

        let json = serde_json::to_string(&settings).unwrap();
        let parsed = serde_json::from_str::<Settings>(&json).unwrap();

        assert_eq!(parsed.mode, Mode::Chase);
        assert_eq!(parsed.speed, 72);
        assert_eq!(parsed.brightness, 64);
        assert!(parsed.running);
        assert_eq!(parsed.f0_color, Rgb { r: 12, g: 34, b: 56 });
        assert_eq!(parsed.window_pos, Some([100.0, 200.0]));
    }

    #[test]
    fn settings_sanitize_clamps_speed_and_drops_bad_position() {
        let settings = Settings {
            mode: Mode::Custom,
            speed: 0,
            brightness: 0,
            running: true,
            f0_color: Rgb::WHITE,
            window_pos: Some([f32::NAN, 10.0]),
        }
        .sanitized();

        assert_eq!(settings.speed, 1);
        assert_eq!(settings.brightness, 1);
        assert!(!settings.running);
        assert_eq!(settings.window_pos, None);
    }

    #[test]
    fn parses_native_color_picker_outputs() {
        assert_eq!(
            parse_color_picker_output("#0c2238\n"),
            Some(Rgb { r: 12, g: 34, b: 56 })
        );
        assert_eq!(
            parse_color_picker_output("rgb(12,34,56)\n"),
            Some(Rgb { r: 12, g: 34, b: 56 })
        );
        assert_eq!(parse_color_picker_output(""), None);
    }

    #[test]
    fn service_generates_chase_colors_for_base_zones() {
        let settings = Settings {
            mode: Mode::Chase,
            speed: 50,
            brightness: 100,
            running: true,
            f0_color: Rgb::WHITE,
            window_pos: None,
        };

        let colors = colors_for_mode(Mode::Chase, 0.0, &settings);

        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0].zone, ZoneId::F0);
        assert_eq!(colors[1].zone, ZoneId::F1);
        assert_eq!(colors[2].zone, ZoneId::F2);
    }
}
