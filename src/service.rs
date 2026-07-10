use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::dchu;
use crate::effects::{colors_for_mode, cycles_per_second, tick_interval};
use crate::led::LedWriter;
use crate::model::{Mode, ZoneColor};
use crate::settings::{
    atomic_write_hardware_snapshot, hardware_snapshot_path, load_settings, service_lock_path,
    service_log_path, service_pid_path,
};

const STATIC_COLOR_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

pub fn ensure_service_running() {
    if active_service_pid().is_some() {
        return;
    }

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("Failed to locate executable for service: {err}");
            return;
        }
    };

    let log_path = service_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();
    let stdout = log
        .as_ref()
        .and_then(|file| file.try_clone().ok())
        .map(Stdio::from)
        .unwrap_or_else(Stdio::null);
    let stderr = log.map(Stdio::from).unwrap_or_else(Stdio::null);

    match Command::new(exe)
        .arg("--service")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
    {
        Ok(child) => {
            let _ = write_pid_file(&service_pid_path(), child.id());
        }
        Err(err) => eprintln!("Failed to start LED service: {err}"),
    }
}

pub fn active_service_pid() -> Option<u32> {
    service_pid()
        .filter(|pid| process_is_running(*pid))
        .or_else(|| read_pid_file(&service_lock_path()).filter(|pid| process_is_running(*pid)))
}

fn service_pid() -> Option<u32> {
    read_pid_file(&service_pid_path())
}

fn read_pid_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
}

fn write_pid_file(path: &Path, pid: u32) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{pid}\n"))
}

fn process_is_running(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

struct ServiceLock {
    _file: fs::File,
}

impl ServiceLock {
    fn acquire() -> io::Result<Self> {
        let path = service_lock_path();
        let pid_path = service_pid_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        for _ in 0..2 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    let pid = std::process::id();
                    fs::write(&path, format!("{pid}\n"))?;
                    let _ = write_pid_file(&pid_path, pid);
                    return Ok(Self { _file: file });
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    if active_service_pid().is_some() {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "service already running",
                        ));
                    }
                    let _ = fs::remove_file(&path);
                    let _ = fs::remove_file(&pid_path);
                }
                Err(err) => return Err(err),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "stale service lock",
        ))
    }
}

impl Drop for ServiceLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(service_lock_path());
        let _ = fs::remove_file(service_pid_path());
    }
}

pub fn service_loop(settings_path: PathBuf) -> ! {
    let lock = match ServiceLock::acquire() {
        Ok(lock) => lock,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            eprintln!("Service is already running: {err}");
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("Failed to acquire service lock: {err}");
            std::process::exit(1);
        }
    };

    let _lock = lock;
    let writer = LedWriter::new();
    let snapshot_path = hardware_snapshot_path();
    let mut phase = 0.0_f32;
    let mut last_hardware_read = Instant::now() - Duration::from_secs(10);
    let mut last_effect_update = Instant::now();
    let mut last_static_refresh = Instant::now() - STATIC_COLOR_REFRESH_INTERVAL;
    let mut last_led_colors: Vec<ZoneColor> = Vec::new();

    loop {
        let now = Instant::now();
        let elapsed = now.duration_since(last_effect_update);
        last_effect_update = now;

        let settings = load_settings(&settings_path);
        if settings.mode == Mode::Custom {
            phase = 0.0;
            last_effect_update = now;
            let colors = colors_for_mode(settings.mode, phase, &settings);
            if static_color_refresh_due(now, last_static_refresh, &colors, &last_led_colors) {
                match writer.write(&colors) {
                    Ok(()) => {
                        last_led_colors = colors;
                        last_static_refresh = now;
                    }
                    Err(err) => {
                        eprintln!("LED service write failed: {err}");
                    }
                }
            }
        } else if settings.running {
            phase = (phase + cycles_per_second(settings.speed) * elapsed.as_secs_f32()).fract();
            let colors = colors_for_mode(settings.mode, phase, &settings);
            if colors != last_led_colors {
                match writer.write(&colors) {
                    Ok(()) => {
                        last_led_colors = colors;
                    }
                    Err(err) => {
                        eprintln!("LED service write failed: {err}");
                    }
                }
            }
        } else {
            last_effect_update = now;
            last_led_colors.clear();
            last_static_refresh = now - STATIC_COLOR_REFRESH_INTERVAL;
        }

        if last_hardware_read.elapsed() >= Duration::from_secs(2) {
            match dchu::read_hardware_snapshot() {
                Ok(snapshot) => {
                    if let Err(err) = atomic_write_hardware_snapshot(&snapshot_path, &snapshot) {
                        eprintln!("Hardware snapshot write failed: {err}");
                    }
                }
                Err(err) => eprintln!("Hardware snapshot read failed: {err}"),
            }
            last_hardware_read = Instant::now();
        }

        thread::sleep(tick_interval(settings.speed));
    }
}

fn static_color_refresh_due(
    now: Instant,
    last_refresh: Instant,
    colors: &[ZoneColor],
    last_colors: &[ZoneColor],
) -> bool {
    colors != last_colors
        || now.saturating_duration_since(last_refresh) >= STATIC_COLOR_REFRESH_INTERVAL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Rgb, ZoneId};

    #[test]
    fn static_color_refresh_runs_when_color_changed() {
        let now = Instant::now();
        let colors = vec![ZoneColor {
            zone: ZoneId::F0,
            rgb: Rgb { r: 1, g: 2, b: 3 },
        }];

        assert!(static_color_refresh_due(
            now,
            now,
            &colors,
            &Vec::<ZoneColor>::new()
        ));
    }

    #[test]
    fn static_color_refresh_runs_on_heartbeat_interval() {
        let now = Instant::now();
        let colors = vec![ZoneColor {
            zone: ZoneId::F0,
            rgb: Rgb { r: 1, g: 2, b: 3 },
        }];
        let last_refresh = now - STATIC_COLOR_REFRESH_INTERVAL;

        assert!(static_color_refresh_due(
            now,
            last_refresh,
            &colors,
            &colors
        ));
    }

    #[test]
    fn static_color_refresh_skips_same_recent_color() {
        let now = Instant::now();
        let colors = vec![ZoneColor {
            zone: ZoneId::F0,
            rgb: Rgb { r: 1, g: 2, b: 3 },
        }];

        assert!(!static_color_refresh_due(now, now, &colors, &colors));
    }
}
