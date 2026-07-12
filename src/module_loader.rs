use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::preferences::UiLanguage;

const REQUIRED_PROC_NODES: [&str; 3] = [
    "/proc/clevo_control_center_led",
    "/proc/clevo_dchu_status",
    "/proc/clevo_dchu_control",
];
const MODULE_VERSION_PROC: &str = "/proc/clevo_control_center_version";
const REQUIRED_MODULE_API_VERSION: u32 = 8;
const MODULE_FILE_NAME: &str = "clevo_control_center.ko";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModuleState {
    Ready,
    Missing,
    Outdated(Option<u32>),
}

pub(crate) fn module_state() -> ModuleState {
    if !REQUIRED_PROC_NODES
        .iter()
        .all(|path| Path::new(path).exists())
    {
        return ModuleState::Missing;
    }

    match module_api_version() {
        Some(version) if version >= REQUIRED_MODULE_API_VERSION => ModuleState::Ready,
        version => ModuleState::Outdated(version),
    }
}

fn module_api_version() -> Option<u32> {
    fs::read_to_string(MODULE_VERSION_PROC)
        .ok()
        .and_then(|text| parse_module_api_version(&text))
}

fn parse_module_api_version(text: &str) -> Option<u32> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("api_version ")
            .and_then(|value| value.trim().parse::<u32>().ok())
    })
}

pub(crate) fn required_module_api_version() -> u32 {
    REQUIRED_MODULE_API_VERSION
}

pub(crate) fn load_module_with_auth(language: UiLanguage) -> Result<(), String> {
    let module_path = module_path_candidate()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let script = r#"
set -eu
PATH=/usr/sbin:/sbin:/usr/bin:/bin
modprobe -r clevo_kbd_led 2>/dev/null || true
modprobe -r clevo_control_center 2>/dev/null || true
rmmod clevo_control_center 2>/dev/null || true
if [ -n "${1:-}" ] && [ -f "$1" ]; then
    insmod "$1"
    exit 0
fi
if modprobe clevo_control_center 2>/dev/null; then
    exit 0
fi
exit 1
"#;

    let output = Command::new("pkexec")
        .arg("sh")
        .arg("-c")
        .arg(script)
        .arg("clevo-control-center-module-loader")
        .arg(module_path)
        .output()
        .map_err(|err| match language {
            UiLanguage::SimplifiedChinese => format!("无法启动 pkexec：{err}"),
            UiLanguage::English => format!("Could not start pkexec: {err}"),
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = format!("{stdout}{stderr}").trim().to_owned();
    if detail.is_empty() {
        Err(match language {
            UiLanguage::SimplifiedChinese => format!("pkexec 返回 {}", output.status),
            UiLanguage::English => format!("pkexec returned {}", output.status),
        })
    } else {
        Err(detail)
    }
}

fn module_path_candidate() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .and_then(|dir| module_path_from_dir(&dir))
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            let parent = exe.parent()?;
            parent.ancestors().find_map(module_path_from_dir)
        })
        .or_else(|| module_path_from_dir(Path::new("/usr/lib/clevo-control-center")))
}

fn module_path_from_dir(dir: &Path) -> Option<PathBuf> {
    let candidate = dir.join("module").join(MODULE_FILE_NAME);
    candidate.exists().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_module_api_version() {
        assert_eq!(parse_module_api_version("api_version 2\n"), Some(2));
        assert_eq!(
            parse_module_api_version("name clevo\napi_version 12\n"),
            Some(12)
        );
        assert_eq!(parse_module_api_version("version 2\n"), None);
        assert_eq!(parse_module_api_version("api_version nope\n"), None);
    }
}
