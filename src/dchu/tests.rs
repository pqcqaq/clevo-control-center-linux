use super::*;

use super::cli::{
    parse_fan_curve_points_arg, parse_fan_mode, parse_gpu_mux_mode, parse_power_mode,
};
use super::io::{fan_rpm_from_tach, parse_dchu_buffer_reply, parse_dchu_config_reply};
use crate::fan_curve::{FanCurve, FanCurvePoint};

#[test]
fn hardware_snapshot_uses_two_primary_fans() {
    let mut bytes = vec![0; 0x20];
    bytes[0x02] = 0x03;
    bytes[0x03] = 0xde;
    bytes[0x04] = 0x04;
    bytes[0x05] = 0x0e;

    let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

    assert_eq!(snapshot.fans.len(), 2);
    assert_eq!(snapshot.fans[0].raw_tach, 990);
    assert_eq!(snapshot.fans[0].rpm, 2178);
    assert_eq!(snapshot.fans[1].raw_tach, 1038);
    assert_eq!(snapshot.fans[1].rpm, 2077);
}

#[test]
fn fan_rpm_uses_inverse_tach_counter_formula() {
    assert_eq!(fan_rpm_from_tach(0), 0);
    assert_eq!(fan_rpm_from_tach(990), 2178);
    assert!(fan_rpm_from_tach(800) > fan_rpm_from_tach(1200));
}

#[test]
fn hardware_snapshot_maps_cpu_and_gpu_temperatures() {
    let mut bytes = vec![0; 0x20];
    bytes[0x10] = 81;
    bytes[0x11] = 43;
    bytes[0x12] = 47;
    bytes[0x13] = 82;
    bytes[0x14] = 48;
    bytes[0x15] = 49;

    let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

    assert_eq!(snapshot.fans[0].temperature_celsius, Some(43));
    assert_eq!(snapshot.fans[1].temperature_celsius, Some(47));
    assert_eq!(snapshot.raw_status, bytes);
    assert_eq!(snapshot.temperature_sensors.len(), 6);
    assert_eq!(snapshot.temperature_sensors[1].label, "CPU 温度");
    assert_eq!(snapshot.temperature_sensors[1].offset, 0x11);
    assert_eq!(snapshot.temperature_sensors[1].raw, 43);
    assert_eq!(snapshot.temperature_sensors[1].celsius, Some(43));
    assert_eq!(snapshot.temperature_sensors[5].offset, 0x15);
    assert_eq!(snapshot.temperature_sensors[5].celsius, Some(49));
}

#[test]
fn hardware_snapshot_diagnostic_report_keeps_decoded_and_raw_values() {
    let mut bytes = vec![0; 0x20];
    bytes[0x02] = 0x03;
    bytes[0x03] = 0xde;
    bytes[0x11] = 43;

    let report = HardwareSnapshot::from_status_bytes(&bytes).diagnostic_report();

    assert!(report.contains("CPU 风扇: 2178 RPM"));
    assert!(report.contains("temperature=43 C"));
    assert!(report.contains("0000:"));
}

#[test]
fn hardware_snapshot_adds_pch_fan_only_when_third_tach_has_data() {
    let mut bytes = vec![0; 0x20];
    let snapshot = HardwareSnapshot::from_status_bytes(&bytes);
    assert_eq!(snapshot.fans.len(), 2);

    bytes[0x06] = 0x07;
    bytes[0x07] = 0x08;
    bytes[0x13] = 51;
    let snapshot = HardwareSnapshot::from_status_bytes(&bytes);

    assert_eq!(snapshot.fans.len(), 3);
    assert_eq!(snapshot.fans[2].label, "PCH 风扇");
    assert_eq!(snapshot.fans[2].rpm, 1197);
    assert_eq!(snapshot.fans[2].temperature_celsius, Some(51));
}

#[test]
fn parses_dchu_fan_modes() {
    assert_eq!(parse_fan_mode("auto").unwrap(), FanMode::Auto);
    assert_eq!(parse_fan_mode("max").unwrap(), FanMode::Max);
    assert_eq!(parse_fan_mode("silent").unwrap(), FanMode::Silent);
    assert_eq!(parse_fan_mode("maxq").unwrap(), FanMode::MaxQ);
    assert_eq!(parse_fan_mode("custom").unwrap(), FanMode::Custom);
    assert_eq!(parse_fan_mode("0").unwrap(), FanMode::Auto);
    assert_eq!(parse_fan_mode("3").unwrap(), FanMode::Silent);
    assert!(parse_fan_mode("2").is_err());
    assert!(parse_fan_mode("7").is_err());
    assert!(parse_fan_mode("turbo").is_err());
    assert!(parse_fan_mode("0x1").is_err());
}

#[test]
fn parses_limited_gpu_mux_modes() {
    assert_eq!(parse_gpu_mux_mode("dgpu").unwrap(), GpuMuxMode::DGpu);
    assert_eq!(parse_gpu_mux_mode("discrete").unwrap(), GpuMuxMode::DGpu);
    assert_eq!(
        parse_gpu_mux_mode("mshybrid").unwrap(),
        GpuMuxMode::MSHybrid
    );
    assert_eq!(parse_gpu_mux_mode("hybrid").unwrap(), GpuMuxMode::MSHybrid);
    assert!(parse_gpu_mux_mode("igpu").is_err());
    assert!(parse_gpu_mux_mode("dds").is_err());
    assert!(parse_gpu_mux_mode("raw").is_err());
}

#[test]
fn parses_dchu_config_reply() {
    let config = parse_dchu_config_reply(
        "config_0d buffer 32\n\
         00 00 00 00 00 00 00 00 00 00 00 00 02 00 00 06\n\
         10 20 30 40 50 60 70 80 00 00 00 00 00 00 00 00\n\
         psf5_10 integer 0x93\n\
         psf1_52 integer 0x4680025\n\
         psf4_60 integer 0x21c\n\
         psf2_7a integer 0x70020053\n\
         bios_feature_04_08_version integer 0x0100\n\
         bios_feature_04_08_offset18 integer 0x01\n\
         gpu_mux_04_15_current integer 0x03\n\
         gpu_mux_04_15_options integer 0x07\n\
         app_power_mode 2\n\
         app_fan_mode 3\n",
    )
    .unwrap();

    assert_eq!(config.fanq, Some(0x02));
    assert_eq!(config.mode_status, Some(0x00));
    assert_eq!(config.kbtp, Some(0x06));
    assert_eq!(config.psf5, Some(0x93));
    assert_eq!(config.psf1, Some(0x0468_0025));
    assert_eq!(config.psf4, Some(0x021c));
    assert_eq!(config.psf2, Some(0x7002_0053));
    assert_eq!(config.bios_feature_version, Some(0x0100));
    assert_eq!(config.bios_feature_offset18, Some(0x01));
    assert_eq!(config.gpu_mux_current, Some(0x03));
    assert_eq!(config.gpu_mux_options, Some(0x07));
    assert_eq!(config.app_power_mode, Some(2));
    assert_eq!(config.app_fan_mode, Some(3));
    assert_eq!(config.raw_config.len(), 32);
}

#[test]
fn keyboard_type_distinguishes_single_three_zone_and_per_key_layouts() {
    let mut config = DchuConfig {
        kbtp: Some(6),
        psf2: Some((1 << 18) | (1 << 12)),
        ..DchuConfig::default()
    };
    assert_eq!(
        config.keyboard_lighting_capabilities(),
        KeyboardLightingCapabilities {
            layout: KeyboardLightingLayout::SingleZone,
            logo: Some(true),
            lightbar: Some(true),
        }
    );

    config.kbtp = Some(2);
    assert_eq!(
        config.keyboard_lighting_capabilities().layout,
        KeyboardLightingLayout::ThreeZone
    );

    for keyboard_type in [3, 19, 35, 51, 243] {
        config.kbtp = Some(keyboard_type);
        assert_eq!(
            config.keyboard_lighting_capabilities().layout,
            KeyboardLightingLayout::PerKey
        );
    }
}

#[test]
fn raw_keyboard_type_takes_precedence_over_compatibility_field() {
    let mut raw_config = vec![0; 16];
    raw_config[15] = 22;
    let config = DchuConfig {
        kbtp: Some(2),
        raw_config,
        ..DchuConfig::default()
    };

    assert_eq!(config.keyboard_type(), Some(22));
    assert_eq!(
        config.keyboard_lighting_capabilities().layout,
        KeyboardLightingLayout::SingleZone
    );
}

#[test]
fn fan_curve_anchors_use_wmi13_first_points_and_oem_duty_conversion() {
    let mut raw_config = vec![0; 34];
    raw_config[16] = 40;
    raw_config[17] = 82;
    raw_config[24] = 42;
    raw_config[25] = 64;
    let config = DchuConfig {
        raw_config,
        ..DchuConfig::default()
    };

    assert_eq!(
        config.cpu_fan_curve_anchor(),
        Some(crate::fan_curve::FanCurvePoint {
            temp_celsius: 40,
            duty_percent: 32,
        })
    );
    assert_eq!(
        config.gpu_fan_curve_anchor(),
        Some(crate::fan_curve::FanCurvePoint {
            temp_celsius: 42,
            duty_percent: 25,
        })
    );
}

#[test]
fn gpu_mux_modes_keep_write_targets_available() {
    let snapshot = HardwareSnapshot {
        fans: Vec::new(),
        temperature_sensors: Vec::new(),
        raw_status: Vec::new(),
        dchu_config: Some(DchuConfig {
            bios_feature_offset18: Some(0x01),
            gpu_mux_current: Some(0x02),
            gpu_mux_options: Some(0x06),
            ..DchuConfig::default()
        }),
        battery_voltage_raw: 0,
        battery_rate_raw: 0,
        thermal_raw: [0; 4],
        updated_unix_secs: 0,
    };

    assert_eq!(
        available_gpu_mux_modes(Some(&snapshot)),
        vec![GpuMuxMode::DGpu, GpuMuxMode::MSHybrid]
    );
    assert_eq!(
        selected_gpu_mux_mode_from_snapshot(Some(&snapshot)),
        Some(GpuMuxMode::DGpu)
    );
}

#[test]
fn gpu_mux_modes_ignore_missing_firmware_options() {
    let snapshot = HardwareSnapshot {
        fans: Vec::new(),
        temperature_sensors: Vec::new(),
        raw_status: Vec::new(),
        dchu_config: Some(DchuConfig {
            psf2: Some(1 << 20),
            ..DchuConfig::default()
        }),
        battery_voltage_raw: 0,
        battery_rate_raw: 0,
        thermal_raw: [0; 4],
        updated_unix_secs: 0,
    };

    assert_eq!(
        available_gpu_mux_modes(Some(&snapshot)),
        vec![GpuMuxMode::DGpu, GpuMuxMode::MSHybrid]
    );
}

#[test]
fn gpu_mux_modes_ignore_incomplete_firmware_options() {
    let snapshot = HardwareSnapshot {
        fans: Vec::new(),
        temperature_sensors: Vec::new(),
        raw_status: Vec::new(),
        dchu_config: Some(DchuConfig {
            gpu_mux_options: Some(0x02),
            ..DchuConfig::default()
        }),
        battery_voltage_raw: 0,
        battery_rate_raw: 0,
        thermal_raw: [0; 4],
        updated_unix_secs: 0,
    };

    assert_eq!(
        available_gpu_mux_modes(Some(&snapshot)),
        vec![GpuMuxMode::DGpu, GpuMuxMode::MSHybrid]
    );
}

#[test]
fn derives_oem_capabilities_from_psf_and_config_buffer() {
    let config = dchu_config_with_raw(0x0000_03e1, 0x0d90_8000, 0x0000_1480, 2, 5, 0x00);

    assert_eq!(config.fan_count(), Some(2));
    assert_eq!(config.init_fan_mode(), Some(5));
    assert_eq!(config.power_mode_capability(), Some(true));
    assert_eq!(config.fan_speed_setting_capability(), Some(true));
    assert_eq!(config.silent_fan_capability(), Some(true));
    assert_eq!(config.maxq_fan_capability(), Some(true));
    assert_eq!(config.custom_fan_table_capability(), Some(true));
    assert_eq!(config.legacy_gpu_mux_capability(), Some(true));
    assert_eq!(config.gpu_mux_capability(), Some(true));
    assert_eq!(config.cpu_oc_capability(), Some(true));
    assert_eq!(config.xmp_capability(), Some(true));
    assert_eq!(config.gpu_oc_capability(), Some(true));
    assert_eq!(config.energy_save_capability(), Some(true));
    assert_eq!(config.battery_utility_capability(), Some(true));
    assert_eq!(config.anti_dust_capability(), Some(true));
    assert_eq!(config.fan_offset_capability(), Some(false));
    assert_eq!(config.dtt_capability(), Some(true));
}

#[test]
fn fan_modes_follow_oem_visibility_bits() {
    let snapshot = snapshot_with_config(dchu_config_with_raw(
        0x0000_0081,
        0x0000_8000,
        0,
        2,
        5,
        0x00,
    ));

    assert_eq!(
        fan_mode_values(&available_fan_modes(Some(&snapshot))),
        vec!["auto", "max", "silent", "maxq"]
    );
}

#[test]
fn silent_fan_mode_is_hidden_without_fanless_capability() {
    let snapshot = snapshot_with_config(dchu_config_with_raw(
        0x0000_0081,
        0x0000_0000,
        0,
        2,
        5,
        0x00,
    ));

    assert_eq!(
        fan_mode_values(&available_fan_modes(Some(&snapshot))),
        vec!["auto", "max", "maxq"]
    );
}

#[test]
fn maxq_fan_mode_is_hidden_without_init_fan_mode_five() {
    let snapshot = snapshot_with_config(dchu_config_with_raw(
        0x0000_0081,
        0x0000_8000,
        0,
        2,
        0,
        0x00,
    ));

    assert_eq!(
        fan_mode_values(&available_fan_modes(Some(&snapshot))),
        vec!["auto", "max", "silent"]
    );
}

#[test]
fn custom_fan_table_capability_does_not_create_write_mode_button() {
    let snapshot = snapshot_with_config(dchu_config_with_raw(
        0x0000_0081,
        0x0000_8000,
        0,
        2,
        5,
        0x00,
    ));

    assert_eq!(
        snapshot
            .dchu_config
            .as_ref()
            .unwrap()
            .custom_fan_table_capability(),
        Some(true)
    );
    assert!(!fan_mode_values(&available_fan_modes(Some(&snapshot))).contains(&"custom"));
}

#[test]
fn fan_mode_controls_hide_when_fan_setting_capability_is_absent() {
    let snapshot = snapshot_with_config(dchu_config_with_raw(
        0x0000_0001,
        0x0000_8000,
        0,
        2,
        5,
        0x00,
    ));

    assert!(available_fan_modes(Some(&snapshot)).is_empty());
}

#[test]
fn power_mode_controls_hide_when_power_capability_is_absent() {
    let snapshot = snapshot_with_config(dchu_config_with_raw(0x0000_0080, 0, 0, 2, 5, 0x00));

    assert!(available_power_modes(Some(&snapshot)).is_empty());
}

#[test]
fn unavailable_config_keeps_safe_control_fallbacks() {
    assert_eq!(
        fan_mode_values(&available_fan_modes(None)),
        vec!["auto", "max"]
    );
    assert_eq!(
        power_mode_values(available_power_modes(None)),
        vec!["0", "1", "2", "3"]
    );
}

#[test]
fn parses_limited_power_modes() {
    assert_eq!(parse_power_mode("0").unwrap(), PowerMode::Quiet);
    assert_eq!(parse_power_mode("3").unwrap(), PowerMode::Entertainment);
    assert!(parse_power_mode("4").is_err());
    assert!(parse_power_mode("0x2").is_err());
    assert!(parse_power_mode("raw-data").is_err());
}

#[test]
fn formats_and_parses_fan_curve_points_arg() {
    let curve = FanCurve {
        points: vec![
            FanCurvePoint {
                temp_celsius: 40,
                duty_percent: 28,
            },
            FanCurvePoint {
                temp_celsius: 58,
                duty_percent: 42,
            },
            FanCurvePoint {
                temp_celsius: 78,
                duty_percent: 72,
            },
            FanCurvePoint {
                temp_celsius: 100,
                duty_percent: 100,
            },
        ],
    };

    let value = fan_curve_points_arg(&curve).unwrap();
    let parsed = parse_fan_curve_points_arg(&value).unwrap();

    assert_eq!(value, "40:28,58:42,78:72,100:100");
    assert_eq!(parsed, curve.points);
}

#[test]
fn rejects_invalid_fan_curve_points_arg() {
    assert!(parse_fan_curve_points_arg("40:20,58:30,80:70").is_err());
    assert!(parse_fan_curve_points_arg("40:20,58:30,58:70,100:100").is_err());
    assert!(parse_fan_curve_points_arg("40:20,58:30,80:10,100:100").is_err());
    assert!(parse_fan_curve_points_arg("40:20,58:30,80:70,120:100").is_err());
}

#[test]
fn selects_fan_mode_from_app_settings_only() {
    let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
    snapshot.dchu_config = Some(DchuConfig {
        mode_status: Some(0x10),
        ..DchuConfig::default()
    });
    assert_eq!(selected_fan_mode_from_snapshot(Some(&snapshot)), None);

    snapshot.dchu_config = Some(DchuConfig {
        mode_status: Some(0x08),
        app_fan_mode: Some(3),
        ..DchuConfig::default()
    });
    assert_eq!(
        selected_fan_mode_from_snapshot(Some(&snapshot)),
        Some(FanMode::Silent)
    );

    snapshot.dchu_config = Some(DchuConfig {
        mode_status: Some(0x02),
        app_fan_mode: Some(5),
        ..DchuConfig::default()
    });
    assert_eq!(
        selected_fan_mode_from_snapshot(Some(&snapshot)),
        Some(FanMode::MaxQ)
    );
}

#[test]
fn selects_power_mode_from_app_settings_only() {
    let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
    snapshot.dchu_config = Some(DchuConfig {
        mode_status: Some(0x80),
        ..DchuConfig::default()
    });
    assert_eq!(selected_power_mode_from_snapshot(Some(&snapshot)), None);

    snapshot.dchu_config = Some(DchuConfig {
        mode_status: Some(0x08),
        app_power_mode: Some(2),
        ..DchuConfig::default()
    });
    assert_eq!(
        selected_power_mode_from_snapshot(Some(&snapshot)),
        Some(PowerMode::Performance)
    );

    snapshot.dchu_config = Some(DchuConfig {
        mode_status: Some(0x02),
        app_power_mode: Some(3),
        ..DchuConfig::default()
    });
    assert_eq!(
        selected_power_mode_from_snapshot(Some(&snapshot)),
        Some(PowerMode::Entertainment)
    );
}

#[test]
fn parses_status_buffer_reply_only() {
    let parsed = parse_dchu_buffer_reply("buffer 4\n01 02 0a ff\n").unwrap();
    assert_eq!(parsed, vec![0x01, 0x02, 0x0a, 0xff]);
    assert!(parse_dchu_buffer_reply("integer 0x79\n").is_err());
}

fn dchu_config_with_raw(
    psf5: u32,
    psf2: u32,
    psf4: u32,
    fan_count: u8,
    init_fan_mode: u8,
    custom_flags: u8,
) -> DchuConfig {
    let mut raw_config = vec![0; 0x2c];
    raw_config[0x0c] = fan_count;
    raw_config[0x0e] = init_fan_mode;
    raw_config[0x2b] = custom_flags;
    DchuConfig {
        fanq: Some(fan_count),
        mode_status: Some(init_fan_mode),
        psf2: Some(psf2),
        psf4: Some(psf4),
        psf5: Some(psf5),
        raw_config,
        ..DchuConfig::default()
    }
}

fn snapshot_with_config(config: DchuConfig) -> HardwareSnapshot {
    let mut snapshot = HardwareSnapshot::from_status_bytes(&[]);
    snapshot.dchu_config = Some(config);
    snapshot
}

fn fan_mode_values(modes: &[FanMode]) -> Vec<&'static str> {
    modes.iter().map(|mode| mode.value()).collect()
}

fn power_mode_values(modes: &[PowerMode]) -> Vec<&'static str> {
    modes.iter().map(|mode| mode.value()).collect()
}
