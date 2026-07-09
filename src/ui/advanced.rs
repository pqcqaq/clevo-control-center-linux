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
        });

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
