use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::hardware;
use crate::model::LightingConfig;
use crate::settings::{
    atomic_write_hardware_snapshot, hardware_snapshot_path, load_settings, service_lock_path,
    service_log_path, service_pid_path,
};

const SERVICE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const LIGHTING_RETRY_INTERVAL: Duration = Duration::from_secs(5);

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
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "Failed to create service log directory {}: {err}",
                parent.display()
            );
        }
    }
    let log = match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(file) => Some(file),
        Err(err) => {
            eprintln!("Failed to open service log {}: {err}", log_path.display());
            None
        }
    };
    let stdout = match log.as_ref() {
        Some(file) => match file.try_clone() {
            Ok(file) => Stdio::from(file),
            Err(err) => {
                eprintln!("Failed to clone service log handle: {err}");
                Stdio::null()
            }
        },
        None => Stdio::null(),
    };
    let stderr = log.map(Stdio::from).unwrap_or_else(Stdio::null);

    match Command::new(exe)
        .arg("--service")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
    {
        Ok(child) => {
            if let Err(err) = write_pid_file(&service_pid_path(), child.id()) {
                eprintln!("Failed to record LED service PID: {err}");
            }
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
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    !matches!(process_state_from_stat(&stat), Some('Z' | 'X') | None)
}

fn process_state_from_stat(stat: &str) -> Option<char> {
    stat.rsplit_once(") ")
        .and_then(|(_, fields)| fields.chars().next())
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
                Ok(mut file) => {
                    let pid = std::process::id();
                    if let Err(err) = writeln!(file, "{pid}") {
                        if let Err(cleanup_err) = fs::remove_file(&path) {
                            eprintln!(
                                "Failed to remove incomplete service lock {}: {cleanup_err}",
                                path.display()
                            );
                        }
                        return Err(err);
                    }
                    if let Err(err) = write_pid_file(&pid_path, pid) {
                        eprintln!(
                            "Failed to record service PID in {}: {err}",
                            pid_path.display()
                        );
                    }
                    return Ok(Self { _file: file });
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    if active_service_pid().is_some_and(|pid| pid != std::process::id()) {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "service already running",
                        ));
                    }
                    if let Err(cleanup_err) = fs::remove_file(&path) {
                        return Err(io::Error::new(
                            cleanup_err.kind(),
                            format!(
                                "failed to remove stale service lock {}: {cleanup_err}",
                                path.display()
                            ),
                        ));
                    }
                    if let Err(cleanup_err) = fs::remove_file(&pid_path) {
                        if cleanup_err.kind() != io::ErrorKind::NotFound {
                            eprintln!(
                                "Failed to remove stale service PID file {}: {cleanup_err}",
                                pid_path.display()
                            );
                        }
                    }
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
        let lock_path = service_lock_path();
        if let Err(err) = fs::remove_file(&lock_path) {
            if err.kind() != io::ErrorKind::NotFound {
                eprintln!(
                    "Failed to remove service lock {}: {err}",
                    lock_path.display()
                );
            }
        }

        let pid_path = service_pid_path();
        if let Err(err) = fs::remove_file(&pid_path) {
            if err.kind() != io::ErrorKind::NotFound {
                eprintln!(
                    "Failed to remove service PID file {}: {err}",
                    pid_path.display()
                );
            }
        }
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
    let hardware = hardware::native_backend();
    let snapshot_path = hardware_snapshot_path();
    let mut last_hardware_read = Instant::now() - Duration::from_secs(10);
    let mut last_lighting: Option<LightingConfig> = None;
    let mut lighting_applied = false;
    let mut last_lighting_attempt = Instant::now() - LIGHTING_RETRY_INTERVAL;

    loop {
        let now = Instant::now();
        let settings = load_settings(&settings_path);
        let lighting = settings.lighting_config();
        let lighting_changed = last_lighting.as_ref() != Some(&lighting);
        let retry_due = !lighting_applied
            && now.saturating_duration_since(last_lighting_attempt) >= LIGHTING_RETRY_INTERVAL;
        if lighting_changed || retry_due {
            last_lighting_attempt = now;
            lighting_applied = match hardware.apply_lighting(&lighting) {
                Ok(()) => true,
                Err(err) => {
                    eprintln!("Lighting service write failed: {err}");
                    false
                }
            };
            last_lighting = Some(lighting);
        }

        if last_hardware_read.elapsed() >= Duration::from_secs(2) {
            match hardware.read_snapshot() {
                Ok(snapshot) => {
                    if let Err(err) = atomic_write_hardware_snapshot(&snapshot_path, &snapshot) {
                        eprintln!("Hardware snapshot write failed: {err}");
                    }
                }
                Err(err) => eprintln!("Hardware snapshot read failed: {err}"),
            }
            last_hardware_read = Instant::now();
        }

        thread::sleep(SERVICE_POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::process_state_from_stat;

    #[test]
    fn proc_stat_parser_distinguishes_running_and_zombie_processes() {
        assert_eq!(
            process_state_from_stat("83487 (clevo-control-center) S 1 2 3"),
            Some('S')
        );
        assert_eq!(
            process_state_from_stat("83492 (clevo-control-center) Z 1 2 3"),
            Some('Z')
        );
        assert_eq!(process_state_from_stat("invalid"), None);
    }
}
