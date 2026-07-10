use eframe::egui::{self, vec2, Color32, Frame, RichText, ScrollArea, Ui};

use crate::dchu::{fan_rpm_from_tach, HardwareSnapshot};

pub(super) fn fan_info(ui: &mut Ui, snapshot: Option<&HardwareSnapshot>) {
    let Some(snapshot) = snapshot else {
        empty(ui);
        return;
    };

    ui.label(
        RichText::new("三路风扇 tach 计数来自 DCHU 0x0C；RPM 使用 2156220 / raw_tach 换算。")
            .size(12.0)
            .color(Color32::from_rgb(151, 145, 135)),
    );
    ui.add_space(8.0);
    egui::Grid::new("advanced-fan-info")
        .striped(true)
        .spacing(vec2(16.0, 7.0))
        .show(ui, |ui| {
            table_header(ui, "通道");
            table_header(ui, "tach offset");
            table_header(ui, "raw tach");
            table_header(ui, "解析 RPM");
            table_header(ui, "温度 offset");
            table_header(ui, "温度");
            ui.end_row();

            for (index, label) in ["CPU 风扇", "GPU 风扇", "PCH 风扇"].into_iter().enumerate()
            {
                let tach_offset = 0x02 + index * 2;
                let temp_offset = 0x11 + index;
                let raw_tach = snapshot_be_u16(snapshot, tach_offset)
                    .or_else(|| snapshot.fans.get(index).map(|fan| fan.raw_tach))
                    .unwrap_or_default();
                let rpm = fan_rpm_from_tach(raw_tach);
                let temp = snapshot
                    .temperature_sensors
                    .iter()
                    .find(|sensor| sensor.offset == temp_offset)
                    .and_then(|sensor| sensor.celsius)
                    .or_else(|| {
                        snapshot
                            .fans
                            .get(index)
                            .and_then(|fan| fan.temperature_celsius)
                    });

                ui.label(label);
                ui.monospace(format!("0x{tach_offset:02x}..0x{:02x}", tach_offset + 1));
                ui.monospace(format!("0x{raw_tach:04x} / {raw_tach}"));
                ui.label(if rpm == 0 {
                    "--".to_owned()
                } else {
                    format!("{rpm} RPM")
                });
                ui.monospace(format!("0x{temp_offset:02x}"));
                ui.label(format_temperature(temp));
                ui.end_row();
            }
        });
}

pub(super) fn temperature_info(ui: &mut Ui, snapshot: Option<&HardwareSnapshot>) {
    let Some(snapshot) = snapshot else {
        empty(ui);
        return;
    };

    let sensors = temperature_rows(snapshot);
    ui.label(
        RichText::new("展示 DCHU 0x0C 中连续温度块 0x10..0x15；未确认含义的只按 offset 展示。")
            .size(12.0)
            .color(Color32::from_rgb(151, 145, 135)),
    );
    ui.add_space(8.0);
    egui::Grid::new("advanced-temperature-info")
        .striped(true)
        .spacing(vec2(18.0, 7.0))
        .show(ui, |ui| {
            table_header(ui, "传感器");
            table_header(ui, "offset");
            table_header(ui, "raw");
            table_header(ui, "摄氏度");
            ui.end_row();

            for (label, offset, raw, celsius) in sensors {
                ui.label(label);
                ui.monospace(format!("0x{offset:02x}"));
                ui.monospace(format!("0x{raw:02x} / {raw}"));
                ui.label(format_temperature(celsius));
                ui.end_row();
            }
        });
}

pub(super) fn other_info(ui: &mut Ui, snapshot: Option<&HardwareSnapshot>) {
    let Some(snapshot) = snapshot else {
        empty(ui);
        return;
    };

    egui::Grid::new("advanced-other-summary")
        .striped(true)
        .spacing(vec2(18.0, 7.0))
        .show(ui, |ui| {
            table_header(ui, "字段");
            table_header(ui, "offset");
            table_header(ui, "raw");
            ui.end_row();
            ui.label("status buffer length");
            ui.monospace("-");
            ui.label(format!("{} bytes", snapshot.raw_status.len()));
            ui.end_row();
            ui.label("battery_voltage_raw");
            ui.monospace("0x08..0x09");
            ui.label(snapshot.battery_voltage_raw.to_string());
            ui.end_row();
            ui.label("battery_rate_raw");
            ui.monospace("0x0e..0x0f");
            ui.label(snapshot.battery_rate_raw.to_string());
            ui.end_row();
            if let Some(config) = &snapshot.dchu_config {
                ui.label("AppSettings power mode");
                ui.monospace("page 1 offset 1");
                ui.label(format_optional_u8(config.app_power_mode));
                ui.end_row();
                ui.label("AppSettings fan mode");
                ui.monospace("page 4 offset 5");
                ui.label(format_optional_u8(config.app_fan_mode));
                ui.end_row();
                ui.label("fanq");
                ui.monospace("0x0d[0x0c]");
                ui.label(format_optional_u8(config.fanq));
                ui.end_row();
                ui.label("kbtp");
                ui.monospace("0x0d[0x0f]");
                ui.label(format_optional_u8(config.kbtp));
                ui.end_row();
                ui.label("psf1_52");
                ui.monospace("0x52");
                ui.label(format_optional_u32_hex(config.psf1));
                ui.end_row();
                ui.label("psf2_7a");
                ui.monospace("0x7a");
                ui.label(format_optional_u32_hex(config.psf2));
                ui.end_row();
                ui.label("psf4_60");
                ui.monospace("0x60");
                ui.label(format_optional_u32_hex(config.psf4));
                ui.end_row();
                ui.label("psf5_10");
                ui.monospace("0x10");
                ui.label(format_optional_u32_hex(config.psf5));
                ui.end_row();
            }
        });

    if let Some(config) = &snapshot.dchu_config {
        ui.add_space(14.0);
        ui.label(
            RichText::new("官方能力位解析")
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(222, 214, 199)),
        );
        ui.add_space(6.0);
        egui::Grid::new("advanced-capability-summary")
            .striped(true)
            .spacing(vec2(18.0, 7.0))
            .show(ui, |ui| {
                table_header(ui, "能力");
                table_header(ui, "来源");
                table_header(ui, "状态");
                ui.end_row();

                capability_row(
                    ui,
                    "电源模式 UI",
                    "PSF5 bit0",
                    config.power_mode_capability(),
                );
                capability_row(
                    ui,
                    "风扇设置 UI",
                    "PSF5 bit7",
                    config.fan_speed_setting_capability(),
                );
                capability_row(
                    ui,
                    "Silent 风扇模式",
                    "PSF2 bit15 FanLess",
                    config.silent_fan_capability(),
                );
                capability_row(
                    ui,
                    "MaxQ 风扇模式",
                    "0x0D[0x0E] InitFanMode == 5",
                    config.maxq_fan_capability(),
                );
                capability_row(
                    ui,
                    "自定义风扇表",
                    "PSF5 bit7 + FanCount + 0x0D[0x2B] bit1",
                    config.custom_fan_table_capability(),
                );
                capability_row(
                    ui,
                    "旧版独显直连/MUX",
                    "PSF2 bit20",
                    config.legacy_gpu_mux_capability(),
                );
                capability_row(
                    ui,
                    "GPU OC",
                    "PSF5 bit5 / PSF2 bit26..27",
                    config.gpu_oc_capability(),
                );
                capability_row(
                    ui,
                    "CPU OC",
                    "PSF5 bit6 / PSF2 bit23",
                    config.cpu_oc_capability(),
                );
                capability_row(ui, "XMP", "PSF2 bit24", config.xmp_capability());
                capability_row(
                    ui,
                    "EnergySave",
                    "PSF5 bit8",
                    config.energy_save_capability(),
                );
                capability_row(
                    ui,
                    "Battery Utility",
                    "PSF5 bit9",
                    config.battery_utility_capability(),
                );
                capability_row(ui, "AntiDust", "PSF4 bit7", config.anti_dust_capability());
                capability_row(
                    ui,
                    "FanOffset",
                    "PSF4 bit10 取反",
                    config.fan_offset_capability(),
                );
                capability_row(ui, "DTT", "PSF4 bit12", config.dtt_capability());

                ui.label("FanCount");
                ui.monospace("0x0D[0x0C]");
                ui.label(format_optional_u8(config.fan_count()));
                ui.end_row();

                ui.label("InitFanMode");
                ui.monospace("0x0D[0x0E]");
                ui.label(format_optional_u8(config.init_fan_mode()));
                ui.end_row();
            });
        ui.add_space(8.0);
        ui.label(
            RichText::new("高级能力目前只读展示；风扇曲线、MUX、超频、电池策略等未做 Linux 写入闭环前不作为控制项公开。")
                .size(12.0)
                .color(Color32::from_rgb(151, 145, 135)),
        );
    }

    ui.add_space(12.0);
    ui.label(
        RichText::new("其他非零 raw byte")
            .size(13.0)
            .strong()
            .color(Color32::from_rgb(222, 214, 199)),
    );
    ui.add_space(6.0);
    ui.monospace(other_nonzero_bytes(snapshot));

    ui.add_space(12.0);
    ui.label(
        RichText::new("完整 DCHU 0x0C raw buffer")
            .size(13.0)
            .strong()
            .color(Color32::from_rgb(222, 214, 199)),
    );
    ui.add_space(6.0);
    Frame::none()
        .fill(Color32::from_rgb(18, 17, 15))
        .rounding(8.0)
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                ui.monospace(status_hex_dump(&snapshot.raw_status));
            });
        });

    if let Some(config) = &snapshot.dchu_config {
        ui.add_space(12.0);
        ui.label(
            RichText::new("完整 DCHU 0x0D config raw buffer")
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(222, 214, 199)),
        );
        ui.add_space(6.0);
        Frame::none()
            .fill(Color32::from_rgb(18, 17, 15))
            .rounding(8.0)
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    ui.monospace(status_hex_dump(&config.raw_config));
                });
            });
    }
}

fn empty(ui: &mut Ui) {
    ui.label(
        RichText::new("暂无硬件状态；等待后台服务或在设置里更新硬件读回。")
            .size(12.0)
            .color(Color32::from_rgb(214, 157, 105)),
    );
}

fn table_header(ui: &mut Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .size(12.0)
            .strong()
            .color(Color32::from_rgb(222, 214, 199)),
    );
}

fn format_temperature(value: Option<u8>) -> String {
    value
        .map(|temp| format!("{temp}°C"))
        .unwrap_or_else(|| "--".to_owned())
}

fn format_optional_u8(value: Option<u8>) -> String {
    value
        .map(|value| format!("0x{value:02x} / {value}"))
        .unwrap_or_else(|| "--".to_owned())
}

fn format_optional_u32_hex(value: Option<u32>) -> String {
    value
        .map(|value| format!("0x{value:08x}"))
        .unwrap_or_else(|| "--".to_owned())
}

fn capability_row(ui: &mut Ui, label: &str, source: &str, value: Option<bool>) {
    ui.label(label);
    ui.monospace(source);
    ui.label(format_capability(value));
    ui.end_row();
}

fn format_capability(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "支持",
        Some(false) => "不支持",
        None => "未知",
    }
}

fn temperature_rows(snapshot: &HardwareSnapshot) -> Vec<(String, usize, u8, Option<u8>)> {
    if !snapshot.temperature_sensors.is_empty() {
        return snapshot
            .temperature_sensors
            .iter()
            .map(|sensor| {
                (
                    sensor.label.clone(),
                    sensor.offset,
                    sensor.raw,
                    sensor.celsius,
                )
            })
            .collect();
    }

    snapshot
        .thermal_raw
        .iter()
        .enumerate()
        .map(|(index, raw)| {
            let offset = 0x10 + index;
            (
                "EC 温度传感器".to_owned(),
                offset,
                *raw,
                plausible_temperature(*raw),
            )
        })
        .collect()
}

fn snapshot_be_u16(snapshot: &HardwareSnapshot, offset: usize) -> Option<u16> {
    let hi = snapshot.raw_status.get(offset).copied()? as u16;
    let lo = snapshot.raw_status.get(offset + 1).copied()? as u16;
    Some((hi << 8) | lo)
}

fn plausible_temperature(value: u8) -> Option<u8> {
    match value {
        1..=125 => Some(value),
        _ => None,
    }
}

fn other_nonzero_bytes(snapshot: &HardwareSnapshot) -> String {
    if snapshot.raw_status.is_empty() {
        return "raw buffer unavailable".to_owned();
    }

    let bytes = snapshot
        .raw_status
        .iter()
        .enumerate()
        .filter(|(offset, value)| **value != 0 && !known_status_byte(*offset))
        .map(|(offset, value)| format!("0x{offset:02x}=0x{value:02x}({value})"))
        .collect::<Vec<_>>();

    if bytes.is_empty() {
        "无".to_owned()
    } else {
        bytes.join("  ")
    }
}

fn known_status_byte(offset: usize) -> bool {
    matches!(offset, 0x02..=0x09 | 0x0e..=0x15)
}

fn status_hex_dump(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "raw buffer unavailable".to_owned();
    }

    let mut lines = Vec::new();
    for (line, chunk) in bytes.chunks(16).enumerate() {
        let offset = line * 16;
        let values = chunk
            .iter()
            .map(|value| format!("{value:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("{offset:02x}: {values}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_helpers_keep_unknown_status_bytes_visible() {
        let mut bytes = vec![0; 0x20];
        bytes[0x02] = 0x03;
        bytes[0x03] = 0xde;
        bytes[0x0a] = 0xa0;
        bytes[0x11] = 43;
        bytes[0x19] = 1;
        let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

        assert_eq!(
            other_nonzero_bytes(&snapshot),
            "0x0a=0xa0(160)  0x19=0x01(1)"
        );
    }

    #[test]
    fn status_hex_dump_formats_offsets() {
        assert_eq!(status_hex_dump(&[0x00, 0x01, 0xff]), "00: 00 01 ff");
        assert_eq!(status_hex_dump(&[]), "raw buffer unavailable");
    }
}
