use crate::fan_curve::{FanCurve, FanCurvePoint, FanCurveProfile};
use crate::hardware::HardwareBackend;

use super::io::{dchu_status_buffer, print_status, read_dchu_config, validate_fan_curve_points};
use super::{FanMode, GpuMuxMode, PowerMode};

fn require_danger_flag(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--i-understand") {
        Ok(())
    } else {
        Err("dangerous write requires --i-understand".to_owned())
    }
}

pub fn print_dchu_usage() {
    println!("Usage:");
    println!("  clevo-control-center dchu status");
    println!("  clevo-control-center dchu app-settings");
    println!(
        "  clevo-control-center dchu fan-mode <auto|max|silent|maxq|custom|0|1|3|5|6> --i-understand"
    );
    println!("  clevo-control-center dchu power-mode <0..3> --i-understand");
    println!(
        "  clevo-control-center dchu fan-curve <cpu t:d,t:d,t:d,t:d> <gpu t:d,t:d,t:d,t:d> --i-understand"
    );
    println!("  clevo-control-center dchu gpu-mux <dgpu|mshybrid> --i-understand");
    println!("  clevo-control-center dchu battery-saver <on|off> --i-understand");
}

pub(super) fn parse_fan_mode(value: &str) -> Result<FanMode, String> {
    FanMode::from_value(value).ok_or_else(|| {
        "fan mode must be one of auto/max/silent/maxq/custom or 0/1/3/5/6".to_owned()
    })
}

fn run_fan_mode(hardware: &dyn HardwareBackend, value: &str) -> Result<(), String> {
    hardware.set_fan_mode(parse_fan_mode(value)?)?;
    println!("fan mode set");
    Ok(())
}

fn run_power_mode(hardware: &dyn HardwareBackend, value: &str) -> Result<(), String> {
    hardware.set_power_mode(parse_power_mode(value)?)?;
    println!("power mode set");
    Ok(())
}

fn run_gpu_mux_mode(hardware: &dyn HardwareBackend, value: &str) -> Result<(), String> {
    let mode = parse_gpu_mux_mode(value)?;
    hardware.set_gpu_mux(mode)?;
    println!("GPU MUX mode set");
    Ok(())
}

fn run_battery_saver(hardware: &dyn HardwareBackend, value: &str) -> Result<(), String> {
    let enabled = match value {
        "on" | "1" => true,
        "off" | "0" => false,
        _ => return Err("battery-saver must be on or off".to_owned()),
    };
    hardware.set_battery_saver(enabled)?;
    println!("battery saver set");
    Ok(())
}

fn run_fan_curve(
    hardware: &dyn HardwareBackend,
    cpu_value: &str,
    gpu_value: &str,
) -> Result<(), String> {
    let profile = FanCurveProfile {
        cpu: FanCurve {
            points: parse_fan_curve_points_arg(cpu_value)?,
        },
        gpu: FanCurve {
            points: parse_fan_curve_points_arg(gpu_value)?,
        },
    };
    hardware.set_fan_curve(&profile)?;
    println!("fan curve set");
    Ok(())
}

fn print_app_settings() -> Result<(), String> {
    let config = read_dchu_config()?;
    println!(
        "power_mode: {}",
        config
            .app_power_mode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    println!(
        "fan_mode: {}",
        config
            .app_fan_mode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    Ok(())
}

pub(super) fn parse_power_mode(value: &str) -> Result<PowerMode, String> {
    PowerMode::from_value(value).ok_or_else(|| "power-mode must be 0..3".to_owned())
}

pub(super) fn parse_gpu_mux_mode(value: &str) -> Result<GpuMuxMode, String> {
    match value {
        "dgpu" | "discrete" | "2" => Ok(GpuMuxMode::DGpu),
        "mshybrid" | "hybrid" | "3" => Ok(GpuMuxMode::MSHybrid),
        _ => Err("gpu-mux must be dgpu or mshybrid".to_owned()),
    }
}

pub(super) fn parse_fan_curve_points_arg(value: &str) -> Result<Vec<FanCurvePoint>, String> {
    let points = value
        .split(',')
        .map(parse_fan_curve_point_arg)
        .collect::<Result<Vec<_>, _>>()?;
    validate_fan_curve_points(&points)?;
    Ok(points)
}

fn parse_fan_curve_point_arg(value: &str) -> Result<FanCurvePoint, String> {
    let Some((temp, duty)) = value.split_once(':') else {
        return Err("fan curve point must use temp:duty".to_owned());
    };
    let temp_celsius = temp
        .parse::<u8>()
        .map_err(|_| "fan curve temperature must be decimal 30..100".to_owned())?;
    let duty_percent = duty
        .parse::<u8>()
        .map_err(|_| "fan curve duty must be decimal 0..100".to_owned())?;

    Ok(FanCurvePoint {
        temp_celsius,
        duty_percent,
    })
}

pub fn run_dchu_cli(args: &[String], hardware: &dyn HardwareBackend) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_dchu_usage();
        return Ok(());
    };

    match command {
        "status" => print_status(&dchu_status_buffer()?),
        "app-settings" => print_app_settings()?,
        "fan-mode" => {
            require_danger_flag(args)?;
            run_fan_mode(hardware, args.get(1).ok_or("fan-mode requires <mode>")?)?;
        }
        "power-mode" => {
            require_danger_flag(args)?;
            run_power_mode(hardware, args.get(1).ok_or("power-mode requires <0..3>")?)?;
        }
        "gpu-mux" => {
            require_danger_flag(args)?;
            run_gpu_mux_mode(
                hardware,
                args.get(1).ok_or("gpu-mux requires <dgpu|mshybrid>")?,
            )?;
        }
        "battery-saver" => {
            require_danger_flag(args)?;
            run_battery_saver(
                hardware,
                args.get(1).ok_or("battery-saver requires <on|off>")?,
            )?;
        }
        "fan-curve" => {
            require_danger_flag(args)?;
            run_fan_curve(
                hardware,
                args.get(1).ok_or("fan-curve requires <cpu points>")?,
                args.get(2).ok_or("fan-curve requires <gpu points>")?,
            )?;
        }
        "help" | "--help" | "-h" => print_dchu_usage(),
        _ => return Err(format!("unknown dchu command: {command}")),
    }

    Ok(())
}
